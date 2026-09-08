#![allow(dead_code)]

use super::error::{ApiError, Result};
use super::models::Me;
use crate::config::Settings;
use parking_lot::RwLock;
use serde::de::DeserializeOwned;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::time::{Duration, Instant};

pub const API_ROOT: &str = "https://api.soundcloud.com";

/// In-flight requests and rate-limit state, shown in the top bar.
///
/// The counter spans a whole logical attempt, retries and back-off waits
/// included, so a request the API is making us wait for still reads as busy.
#[derive(Debug, Default)]
pub struct NetActivity {
    in_flight: AtomicU32,
    /// When the oldest current request started, as ms since `epoch`.
    started_ms: AtomicU64,
    /// When the 429 cool-down ends, as ms since `epoch`; 0 when clear.
    cooldown_until_ms: AtomicU64,
    epoch: RwLock<Option<Instant>>,
}

impl NetActivity {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            epoch: RwLock::new(Some(Instant::now())),
            ..Default::default()
        })
    }

    fn now_ms(&self) -> u64 {
        let epoch = self.epoch.read().unwrap_or_else(Instant::now);
        epoch.elapsed().as_millis() as u64
    }

    /// Mark a logical request as started.
    pub fn begin(&self) {
        if self.in_flight.fetch_add(1, Ordering::SeqCst) == 0 {
            self.started_ms.store(self.now_ms(), Ordering::SeqCst);
        }
    }

    pub fn end(&self) {
        let _ = self
            .in_flight
            .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |n| {
                Some(n.saturating_sub(1))
            });
    }

    /// Something has been in flight for longer than `threshold`.
    pub fn busy(&self, threshold: Duration) -> bool {
        if self.in_flight.load(Ordering::SeqCst) == 0 {
            return false;
        }
        let started = self.started_ms.load(Ordering::SeqCst);
        self.now_ms().saturating_sub(started) >= threshold.as_millis() as u64
    }

    pub fn in_flight(&self) -> u32 {
        self.in_flight.load(Ordering::SeqCst)
    }

    /// Note a 429: requests hold off until `wait` has passed.
    pub fn rate_limited(&self, wait: Duration) {
        let until = self.now_ms() + wait.as_millis() as u64;
        self.cooldown_until_ms.fetch_max(until, Ordering::SeqCst);
    }

    /// Remaining cool-down, if the API asked us to wait.
    pub fn cooldown_left(&self) -> Option<Duration> {
        let until = self.cooldown_until_ms.load(Ordering::SeqCst);
        if until == 0 {
            return None;
        }
        let now = self.now_ms();
        (until > now).then(|| Duration::from_millis(until - now))
    }
}

/// Rate-limit-aware HTTP client for api.soundcloud.com.
pub struct ApiClient {
    http: reqwest::Client,
    client_id: RwLock<Option<String>>,
    oauth: RwLock<Option<String>>,
    session: RwLock<std::sync::Weak<crate::auth::Session>>,
    /// Whether to answer from the offline demo library instead of the network.
    ///
    /// Atomic rather than a plain `bool` because the client is shared behind an
    /// `Arc` (the player holds one) and registering an application mid-session
    /// has to switch the app to the live API without rebuilding everything
    /// that points at it.
    demo: std::sync::atomic::AtomicBool,
    activity: Arc<NetActivity>,
}

impl ApiClient {
    pub fn new(client_id: Option<String>, demo: bool) -> Self {
        let http = reqwest::Client::builder()
            .user_agent(concat!("fastcloud/", env!("CARGO_PKG_VERSION")))
            .timeout(Duration::from_secs(30))
            .build()
            .unwrap_or_else(|error| {
                log::warn!("could not configure HTTP client, using defaults: {error}");
                reqwest::Client::new()
            });
        Self {
            http,
            client_id: RwLock::new(client_id),
            oauth: RwLock::new(None),
            session: RwLock::new(std::sync::Weak::new()),
            demo: std::sync::atomic::AtomicBool::new(demo),
            activity: NetActivity::new(),
        }
    }

    pub fn demo() -> Self {
        Self::new(None, true)
    }

    /// Shared network state for the top bar's spinner.
    pub fn activity(&self) -> Arc<NetActivity> {
        self.activity.clone()
    }

    pub fn set_oauth(&self, token: String) {
        *self.oauth.write() = Some(token);
    }

    pub fn oauth_token(&self) -> Option<String> {
        self.oauth.read().clone()
    }

    pub fn clear_oauth(&self) {
        *self.oauth.write() = None;
    }

    pub fn attach_session(&self, session: &Arc<crate::auth::Session>) {
        *self.session.write() = Arc::downgrade(session);
    }

    pub fn set_client_id(&self, id: String) {
        *self.client_id.write() = Some(id);
    }

    pub fn is_demo(&self) -> bool {
        self.demo.load(std::sync::atomic::Ordering::Relaxed)
    }

    /// Leave (or enter) demo mode. Registering an application is the way out of
    /// it without a restart.
    pub fn set_demo(&self, demo: bool) {
        self.demo.store(demo, std::sync::atomic::Ordering::Relaxed);
    }

    /// Perform a GET request with automatic 429 retry honoring Retry-After.
    pub async fn get<T: DeserializeOwned>(
        &self,
        path: &str,
        query: &[(&str, String)],
    ) -> Result<T> {
        let url = format!("{}{}", API_ROOT, path);
        self.get_url(&url, query).await
    }

    pub async fn get_url<T: DeserializeOwned>(
        &self,
        url: &str,
        query: &[(&str, String)],
    ) -> Result<T> {
        self.request_json(reqwest::Method::GET, url, query, None)
            .await
    }

    pub async fn post<T: DeserializeOwned>(
        &self,
        path: &str,
        query: &[(&str, String)],
        body: Option<serde_json::Value>,
    ) -> Result<T> {
        let url = format!("{}{}", API_ROOT, path);
        self.request_json(reqwest::Method::POST, &url, query, body)
            .await
    }

    pub async fn put<T: DeserializeOwned>(
        &self,
        path: &str,
        query: &[(&str, String)],
        body: Option<serde_json::Value>,
    ) -> Result<T> {
        let url = format!("{}{}", API_ROOT, path);
        self.request_json(reqwest::Method::PUT, &url, query, body)
            .await
    }

    pub async fn delete(&self, path: &str, query: &[(&str, String)]) -> Result<()> {
        let url = format!("{}{}", API_ROOT, path);
        self.activity.begin();
        let out = self.delete_inner(&url, query).await;
        self.activity.end();
        out
    }

    async fn delete_inner(&self, url: &str, query: &[(&str, String)]) -> Result<()> {
        self.await_cooldown().await;
        let resp = self.send(reqwest::Method::DELETE, url, query, None).await?;
        let status = resp.status();
        if status.is_success() || status == reqwest::StatusCode::NOT_FOUND {
            Ok(())
        } else {
            Err(ApiError::Http {
                status: status.as_u16(),
                body: resp.text().await.unwrap_or_default(),
            })
        }
    }

    pub async fn get_me(&self) -> Result<Me> {
        self.get("/me", &[]).await
    }

    /// Perform a raw GET and return bytes (used for cover art).
    pub async fn raw_get(&self, url: &str) -> Result<bytes::Bytes> {
        let resp = self.http.get(url).send().await.map_err(ApiError::from)?;
        let status = resp.status();
        if !status.is_success() {
            return Err(ApiError::Http {
                status: status.as_u16(),
                body: String::new(),
            });
        }
        Ok(resp.bytes().await?)
    }

    /// Upload one file without buffering it in RAM. The multipart body is
    /// rebuilt after a 429 because reqwest bodies are intentionally one-shot.
    pub async fn post_multipart_file<T: DeserializeOwned>(
        &self,
        path: &str,
        fields: &[(String, String)],
        file_field: &str,
        file_path: &Path,
    ) -> Result<T> {
        let url = format!("{}{}", API_ROOT, path);
        self.activity.begin();
        let out = self
            .post_multipart_file_inner(&url, fields, file_field, file_path)
            .await;
        self.activity.end();
        out
    }

    async fn post_multipart_file_inner<T: DeserializeOwned>(
        &self,
        url: &str,
        fields: &[(String, String)],
        file_field: &str,
        file_path: &Path,
    ) -> Result<T> {
        let mut attempt = 0u32;
        loop {
            self.await_cooldown().await;
            let session = self.session.read().upgrade();
            if let Some(session) = session {
                session.access_token().await.map_err(ApiError::Other)?;
            }
            let mut form = reqwest::multipart::Form::new();
            for (name, value) in fields {
                form = form.text(name.clone(), value.clone());
            }
            form = form
                .file(file_field.to_owned(), file_path)
                .await
                .map_err(anyhow::Error::from)?;
            let mut request = self
                .http
                .post(url)
                .header(reqwest::header::ACCEPT, "application/json; charset=utf-8")
                .multipart(form);
            if let Some(token) = self.oauth.read().as_ref() {
                request = request.header(reqwest::header::AUTHORIZATION, format!("OAuth {token}"));
            }
            let response = request.send().await.map_err(ApiError::from)?;
            if response.status() == reqwest::StatusCode::TOO_MANY_REQUESTS && attempt < 3 {
                let wait_ms = response
                    .headers()
                    .get("Retry-After")
                    .and_then(|value| value.to_str().ok())
                    .and_then(|value| value.parse::<u64>().ok())
                    .map(|seconds| seconds * 1000)
                    .unwrap_or(2_000)
                    .min(30_000);
                self.activity.rate_limited(Duration::from_millis(wait_ms));
                attempt += 1;
                tokio::time::sleep(Duration::from_millis(wait_ms)).await;
                continue;
            }
            let status = response.status();
            if status == reqwest::StatusCode::UNAUTHORIZED {
                return Err(ApiError::Unauthorized);
            }
            let bytes = response.bytes().await?;
            if !status.is_success() {
                return Err(ApiError::Http {
                    status: status.as_u16(),
                    body: String::from_utf8_lossy(&bytes).into_owned(),
                });
            }
            return decode_response(&bytes);
        }
    }

    /// Resolve a redirecting API resource (currently track previews) to the
    /// final CDN URL while keeping OAuth on the API request itself.
    pub async fn redirected_url(&self, path: &str, query: &[(String, String)]) -> Result<String> {
        let url = format!("{}{}", API_ROOT, path);
        self.activity.begin();
        self.await_cooldown().await;
        let pairs: Vec<_> = query
            .iter()
            .map(|(key, value)| (key.as_str(), value.clone()))
            .collect();
        let response = self.send(reqwest::Method::GET, &url, &pairs, None).await;
        self.activity.end();
        let response = response?;
        if !response.status().is_success() {
            return Err(ApiError::Http {
                status: response.status().as_u16(),
                body: response.text().await.unwrap_or_default(),
            });
        }
        Ok(response.url().to_string())
    }

    async fn request_json<T: DeserializeOwned>(
        &self,
        method: reqwest::Method,
        url: &str,
        query: &[(&str, String)],
        body: Option<serde_json::Value>,
    ) -> Result<T> {
        // One `begin`/`end` pair for the whole attempt, retries included, so
        // the interface can say "waiting for SoundCloud" while we back off.
        self.activity.begin();
        let out = self.request_json_inner(method, url, query, body).await;
        self.activity.end();
        out
    }

    async fn request_json_inner<T: DeserializeOwned>(
        &self,
        method: reqwest::Method,
        url: &str,
        query: &[(&str, String)],
        body: Option<serde_json::Value>,
    ) -> Result<T> {
        let mut attempt = 0u32;
        loop {
            self.await_cooldown().await;
            let resp = self.send(method.clone(), url, query, body.clone()).await?;
            let status = resp.status();
            if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
                let retry_after = resp
                    .headers()
                    .get("Retry-After")
                    .and_then(|v| v.to_str().ok())
                    .and_then(|v| v.parse::<u64>().ok());
                let wait_ms = retry_after.map(|s| s * 1000).unwrap_or(2000u64).min(30_000);
                self.activity.rate_limited(Duration::from_millis(wait_ms));
                attempt += 1;
                if attempt > 3 {
                    return Err(ApiError::RateLimited {
                        retry_after_ms: Some(wait_ms),
                    });
                }
                tokio::time::sleep(Duration::from_millis(wait_ms)).await;
                continue;
            }
            if status == reqwest::StatusCode::UNAUTHORIZED {
                return Err(ApiError::Unauthorized);
            }
            if !status.is_success() {
                return Err(ApiError::Http {
                    status: status.as_u16(),
                    body: resp.text().await.unwrap_or_default(),
                });
            }
            let bytes = resp.bytes().await?;
            return decode_response(&bytes);
        }
    }

    /// Hold off while a 429 cool-down is running.
    async fn await_cooldown(&self) {
        if let Some(left) = self.activity.cooldown_left() {
            tokio::time::sleep(left).await;
        }
    }

    async fn send(
        &self,
        method: reqwest::Method,
        url: &str,
        query: &[(&str, String)],
        body: Option<serde_json::Value>,
    ) -> Result<reqwest::Response> {
        // The session serializes refreshes; the weak link avoids a reference
        // cycle while ensuring playback and every catalogue request stay fresh.
        let session = self.session.read().upgrade();
        if let Some(session) = session {
            session.access_token().await.map_err(ApiError::Other)?;
        }
        let mut req = self.http.request(method, url);
        req = req.header(reqwest::header::ACCEPT, "application/json; charset=utf-8");
        if !query.is_empty() {
            req = req.query(query);
        }
        // SoundCloud wants `Authorization: OAuth <token>`, not Bearer
        // (API Guide → Authentication).
        if let Some(token) = self.oauth.read().as_ref() {
            req = req.header(reqwest::header::AUTHORIZATION, format!("OAuth {token}"));
        }
        if let Some(body) = body {
            req = req.json(&body);
        }
        req.send().await.map_err(ApiError::from)
    }
}

fn decode_response<T: DeserializeOwned>(bytes: &[u8]) -> Result<T> {
    // Reposts and follows can succeed with 201/204 and no JSON body.
    // Only types accepting null (Value, (), Option) accept this success;
    // a missing track or collection still fails loudly.
    let bytes = if bytes.iter().all(u8::is_ascii_whitespace) {
        b"null"
    } else {
        bytes
    };
    Ok(serde_json::from_slice(bytes)?)
}

pub type SharedClient = Arc<ApiClient>;

pub fn from_settings(settings: &Settings, demo: bool) -> SharedClient {
    Arc::new(ApiClient::new(settings.client_id.clone(), demo))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_empty_success_is_valid_for_writes_but_not_for_tracks() {
        assert_eq!(
            decode_response::<serde_json::Value>(b"").unwrap(),
            serde_json::Value::Null
        );
        assert!(decode_response::<crate::api::models::Track>(b"").is_err());
        assert!(decode_response::<serde_json::Value>(b"{").is_err());
    }

    #[test]
    fn activity_counts_overlapping_requests() {
        let net = NetActivity::new();
        assert_eq!(net.in_flight(), 0);
        assert!(!net.busy(Duration::ZERO));
        net.begin();
        net.begin();
        assert_eq!(net.in_flight(), 2);
        // Busy right away at a zero threshold, quiet at a long one.
        assert!(net.busy(Duration::ZERO));
        assert!(!net.busy(Duration::from_secs(5)));
        net.end();
        assert_eq!(net.in_flight(), 1);
        net.end();
        assert_eq!(net.in_flight(), 0);
        assert!(!net.busy(Duration::ZERO));
        // Never underflows.
        net.end();
        assert_eq!(net.in_flight(), 0);
    }

    #[test]
    fn cooldown_reports_time_left_and_keeps_the_longest() {
        let net = NetActivity::new();
        assert!(net.cooldown_left().is_none());
        net.rate_limited(Duration::from_secs(30));
        let left = net.cooldown_left().expect("cooling down");
        assert!(left <= Duration::from_secs(30) && left > Duration::from_secs(25));
        // A shorter wait must not shorten an existing one.
        net.rate_limited(Duration::from_secs(1));
        let left = net.cooldown_left().expect("still cooling down");
        assert!(left > Duration::from_secs(25));
    }

    /// Registering an application has to switch a running app to the live API,
    /// and the player holds this client behind an `Arc` — so demo mode is a
    /// shared flag that can be turned off in place rather than a field fixed at
    /// construction.
    #[test]
    fn demo_mode_can_be_left_without_rebuilding_the_client() {
        let client = ApiClient::demo();
        assert!(client.is_demo());
        client.set_demo(false);
        assert!(!client.is_demo(), "a registration could not take effect");
        // …and a shared reference is enough to do it, which is the point.
        let shared = Arc::new(client);
        let other = shared.clone();
        other.set_demo(true);
        assert!(shared.is_demo());
        other.set_demo(false);
        assert!(!shared.is_demo());
        // A client built for the live API starts out of demo mode.
        assert!(!ApiClient::new(Some("id".into()), false).is_demo());
    }
}
