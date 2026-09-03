#![allow(dead_code)]

use super::error::{ApiError, Result};
use super::models::Me;
use crate::config::Settings;
use parking_lot::RwLock;
use serde::de::DeserializeOwned;
use std::sync::Arc;
use std::time::Duration;

pub const API_ROOT: &str = "https://api.soundcloud.com";

/// Rate-limit-aware HTTP client for api.soundcloud.com.
pub struct ApiClient {
    http: reqwest::Client,
    client_id: RwLock<Option<String>>,
    oauth: RwLock<Option<String>>,
    demo: bool,
}

impl ApiClient {
    pub fn new(client_id: Option<String>, demo: bool) -> Self {
        let http = reqwest::Client::builder()
            .user_agent(concat!("fastcloud/", env!("CARGO_PKG_VERSION")))
            .timeout(Duration::from_secs(30))
            .build()
            .expect("reqwest client");
        Self {
            http,
            client_id: RwLock::new(client_id),
            oauth: RwLock::new(None),
            demo,
        }
    }

    pub fn demo() -> Self {
        Self::new(None, true)
    }

    pub fn set_oauth(&self, token: String) {
        *self.oauth.write() = Some(token);
    }

    pub fn clear_oauth(&self) {
        *self.oauth.write() = None;
    }

    pub fn set_client_id(&self, id: String) {
        *self.client_id.write() = Some(id);
    }

    pub fn is_demo(&self) -> bool {
        self.demo
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
        let resp = self
            .send(reqwest::Method::DELETE, &url, query, None)
            .await?;
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

    async fn request_json<T: DeserializeOwned>(
        &self,
        method: reqwest::Method,
        url: &str,
        query: &[(&str, String)],
        body: Option<serde_json::Value>,
    ) -> Result<T> {
        let mut attempt = 0u32;
        loop {
            let resp = self.send(method.clone(), url, query, body.clone()).await?;
            let status = resp.status();
            if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
                let retry_after = resp
                    .headers()
                    .get("Retry-After")
                    .and_then(|v| v.to_str().ok())
                    .and_then(|v| v.parse::<u64>().ok());
                let wait_ms = retry_after.map(|s| s * 1000).unwrap_or(2000u64).min(30_000);
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
            return Ok(serde_json::from_slice(&bytes)?);
        }
    }

    async fn send(
        &self,
        method: reqwest::Method,
        url: &str,
        query: &[(&str, String)],
        body: Option<serde_json::Value>,
    ) -> Result<reqwest::Response> {
        let mut req = self.http.request(method, url);
        if !query.is_empty() {
            req = req.query(query);
        }
        if let Some(token) = self.oauth.read().as_ref() {
            req = req.bearer_auth(token);
        }
        if let Some(body) = body {
            req = req.json(&body);
        }
        req.send().await.map_err(ApiError::from)
    }
}

pub type SharedClient = Arc<ApiClient>;

pub fn from_settings(settings: &Settings, demo: bool) -> SharedClient {
    Arc::new(ApiClient::new(settings.client_id.clone(), demo))
}
