//! SoundCloud OAuth 2.1, exactly as the API Guide specifies it.
//!
//! Two flows, both against `https://secure.soundcloud.com`:
//!
//! * **Authorization Code + PKCE** ([`authorize`]) for anything touching the
//!   account — likes, playlists, the feed, `/me`.
//! * **Client Credentials** ([`client_credentials`]) for public browsing:
//!   search, `/resolve`, and playback of public tracks. No user session, so
//!   the app is useful before anyone signs in.
//!
//! ## Why a client secret is required
//!
//! PKCE normally lets a native app skip the secret. SoundCloud does not:
//!
//! > All clients are currently treated as confidential rather than public,
//! > meaning a secret is required to obtain a token.
//!
//! So the token exchange sends `client_secret` as well as the PKCE verifier,
//! and Fastcloud can ship **no** credentials of its own: each user needs an
//! application. [`register`] gets them one without their leaving the app — one
//! browser sign-in registers it and hands back the keys, which also proves the
//! account has the Artist Pro subscription SoundCloud requires for API access.
//! The keys go to the OS keyring, never to a file in the repo.
//!
//! ## Token lifetime
//!
//! Access tokens last about an hour and refresh tokens are **single-use**, so
//! a refresh must persist the new pair before anything else can use it;
//! [`Session`] serialises refreshes behind one lock so two callers racing a
//! 401 cannot burn the same refresh token twice.

#![allow(dead_code)]

pub mod register;

use anyhow::{Context, Result};
use base64::Engine;
use rand::RngCore;
use std::net::TcpListener;
use std::sync::Arc;

/// Loopback port the browser is redirected back to.
///
/// The redirect URI must match the registered one **exactly**, so this is
/// only the default: [`AppCredentials::redirect_uri`] carries whatever the
/// user registered.
pub const AUTH_PORT: u16 = 41317;
pub const DEFAULT_REDIRECT_URI: &str = "http://127.0.0.1:41317/callback";

pub const AUTH_ROOT: &str = "https://secure.soundcloud.com";

pub const AUTH_SERVICE: &str = "com.fastcloud.tokens";
pub const USER_ENTRY: &str = "oauth";
pub const CLIENT_ID_ENTRY: &str = "client_id";
pub const CLIENT_SECRET_ENTRY: &str = "client_secret";
pub const REDIRECT_URI_ENTRY: &str = "redirect_uri";

/// Refresh this long before the token actually expires, so a request never
/// starts with a token that dies mid-flight.
const REFRESH_SKEW_SECS: i64 = 120;

/// How long to wait for the browser to come back.
const CALLBACK_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(300);

// ===========================================================================
// PKCE
// ===========================================================================

/// A PKCE verifier and its S256 challenge.
///
/// The guide asks for `code_challenge_method=S256`; `plain` is what OAuth 2.0
/// tolerated and 2.1 removed.
#[derive(Debug, Clone)]
pub struct Pkce {
    pub verifier: String,
}

impl Pkce {
    /// 43–128 characters of unreserved ASCII, per RFC 7636. 32 random bytes
    /// base64url-encode to 43, the shortest the spec allows.
    pub fn generate() -> Self {
        let mut bytes = [0u8; 32];
        rand::rng().fill_bytes(&mut bytes);
        Self {
            verifier: b64(&bytes),
        }
    }

    /// `BASE64URL(SHA256(verifier))`, no padding.
    pub fn challenge(&self) -> String {
        use sha2::Digest;
        b64(&sha2::Sha256::digest(self.verifier.as_bytes()))
    }
}

fn b64(bytes: &[u8]) -> String {
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes)
}

// ===========================================================================
// Credentials
// ===========================================================================

/// The user's own registered application.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppCredentials {
    pub client_id: String,
    pub client_secret: String,
    /// Must match the app registration character for character.
    pub redirect_uri: String,
}

impl AppCredentials {
    pub fn new(client_id: String, client_secret: String, redirect_uri: Option<String>) -> Self {
        Self {
            client_id,
            client_secret,
            redirect_uri: redirect_uri.unwrap_or_else(|| DEFAULT_REDIRECT_URI.to_owned()),
        }
    }

    /// From the environment (CI, scripts, `FASTCLOUD_REDIRECT_URI` optional).
    pub fn from_env() -> Option<Self> {
        Some(Self::new(
            std::env::var("FASTCLOUD_CLIENT_ID").ok()?,
            std::env::var("FASTCLOUD_CLIENT_SECRET").ok()?,
            std::env::var("FASTCLOUD_REDIRECT_URI").ok(),
        ))
    }

    /// From the OS keyring, where Settings writes them.
    pub fn from_keyring() -> Option<Self> {
        Some(Self::new(
            read_secret(CLIENT_ID_ENTRY)?,
            read_secret(CLIENT_SECRET_ENTRY)?,
            read_secret(REDIRECT_URI_ENTRY),
        ))
    }

    /// Environment first (explicit beats stored), then the keyring.
    pub fn discover() -> Option<Self> {
        Self::from_env().or_else(Self::from_keyring)
    }

    pub fn save_to_keyring(&self) -> Result<()> {
        write_secret(CLIENT_ID_ENTRY, &self.client_id)?;
        write_secret(CLIENT_SECRET_ENTRY, &self.client_secret)?;
        write_secret(REDIRECT_URI_ENTRY, &self.redirect_uri)?;
        Ok(())
    }

    pub fn forget() -> Result<()> {
        for entry in [CLIENT_ID_ENTRY, CLIENT_SECRET_ENTRY, REDIRECT_URI_ENTRY] {
            delete_secret(entry)?;
        }
        Ok(())
    }

    /// The browser URL that starts the authorization-code flow.
    pub fn authorize_url(&self, pkce: &Pkce, state: &str) -> String {
        let mut url = format!("{AUTH_ROOT}/authorize");
        let query = [
            ("client_id", self.client_id.as_str()),
            ("redirect_uri", self.redirect_uri.as_str()),
            ("response_type", "code"),
            ("code_challenge", &pkce.challenge()),
            ("code_challenge_method", "S256"),
            ("state", state),
        ];
        for (i, (key, value)) in query.iter().enumerate() {
            url.push(if i == 0 { '?' } else { '&' });
            url.push_str(key);
            url.push('=');
            url.push_str(&form_encode(value));
        }
        url
    }

    /// `Authorization: Basic base64(id:secret)`, the only form the
    /// client-credentials grant accepts.
    fn basic_auth(&self) -> String {
        let raw = format!("{}:{}", self.client_id, self.client_secret);
        format!(
            "Basic {}",
            base64::engine::general_purpose::STANDARD.encode(raw)
        )
    }

    /// The loopback port to listen on, read back out of the redirect URI so a
    /// user who registered a different port still works.
    pub fn callback_port(&self) -> u16 {
        self.redirect_uri
            .split("//")
            .nth(1)
            .and_then(|rest| rest.split('/').next())
            .and_then(|host| host.rsplit(':').next())
            .and_then(|port| port.parse().ok())
            .unwrap_or(AUTH_PORT)
    }

    /// Whether the redirect URI is a loopback address this app can serve.
    /// A custom scheme (`fastcloud://…`) would need OS registration instead.
    pub fn redirect_is_loopback(&self) -> bool {
        self.redirect_uri.starts_with("http://127.0.0.1")
            || self.redirect_uri.starts_with("http://localhost")
            || self.redirect_uri.starts_with("http://[::1]")
    }
}

fn read_secret(key: &str) -> Option<String> {
    keyring::Entry::new(AUTH_SERVICE, key)
        .ok()?
        .get_password()
        .ok()
        .filter(|value| !value.is_empty())
}

fn write_secret(key: &str, value: &str) -> Result<()> {
    keyring::Entry::new(AUTH_SERVICE, key)
        .with_context(|| format!("open keyring entry {key}"))?
        .set_password(value)
        .with_context(|| format!("store {key}"))
}

fn delete_secret(key: &str) -> Result<()> {
    let entry = keyring::Entry::new(AUTH_SERVICE, key).context("open keyring entry")?;
    match entry.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(e) => Err(anyhow::anyhow!("keyring: {e}")),
    }
}

// ===========================================================================
// Tokens
// ===========================================================================

/// Which flow minted a token. A client-credentials token cannot read `/me`,
/// so the UI needs to know which one it holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Grant {
    /// Signed in: the account's own data is reachable.
    User,
    /// App-only: public resources.
    App,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Tokens {
    pub access_token: String,
    /// Empty when the response carried none (client credentials may not).
    #[serde(default)]
    pub refresh_token: String,
    #[serde(default)]
    pub expires_at: Option<i64>,
    #[serde(default)]
    pub scope: Option<String>,
    #[serde(default = "user_grant")]
    pub grant: Grant,
}

fn user_grant() -> Grant {
    Grant::User
}

impl Tokens {
    pub(crate) fn from_response(body: &serde_json::Value, grant: Grant) -> Result<Self> {
        let access_token = body
            .get("access_token")
            .and_then(|v| v.as_str())
            .filter(|token| !token.is_empty())
            .context("token response carried no access_token")?
            .to_owned();
        let expires_in = body
            .get("expires_in")
            .and_then(|v| v.as_i64())
            .unwrap_or(3600);
        Ok(Self {
            access_token,
            refresh_token: body
                .get("refresh_token")
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_owned(),
            expires_at: Some(chrono::Utc::now().timestamp() + expires_in),
            scope: body
                .get("scope")
                .and_then(|v| v.as_str())
                .map(str::to_owned),
            grant,
        })
    }

    /// Expired, or close enough that a request would race the expiry.
    pub fn expired(&self) -> bool {
        self.expires_at
            .map(|at| chrono::Utc::now().timestamp() >= at - REFRESH_SKEW_SECS)
            .unwrap_or(true)
    }

    pub fn can_refresh(&self) -> bool {
        !self.refresh_token.is_empty()
    }

    // ===== Keyring persistence =====

    pub fn save(&self) -> Result<()> {
        write_secret(USER_ENTRY, &serde_json::to_string(self)?)
    }

    pub fn load() -> Result<Option<Self>> {
        let entry = keyring::Entry::new(AUTH_SERVICE, USER_ENTRY).context("keyring entry")?;
        match entry.get_password() {
            Ok(json) => Ok(serde_json::from_str(&json).ok()),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(e) => Err(anyhow::anyhow!("keyring: {e}")),
        }
    }

    pub fn delete() -> Result<()> {
        delete_secret(USER_ENTRY)
    }
}

// ===========================================================================
// Session
// ===========================================================================

/// Holds the live tokens and keeps them fresh.
///
/// Refresh tokens are single-use, so every refresh goes through one async
/// mutex: two views hitting a stale token at once would otherwise both post
/// the same refresh token and the second would be rejected, signing the user
/// out for no reason.
pub struct Session {
    creds: AppCredentials,
    tokens: tokio::sync::Mutex<Option<Tokens>>,
    client: Arc<crate::api::ApiClient>,
    http: reqwest::Client,
}

impl Session {
    pub fn new(creds: AppCredentials, client: Arc<crate::api::ApiClient>) -> Arc<Self> {
        let session = Arc::new(Self {
            creds,
            tokens: tokio::sync::Mutex::new(None),
            client: client.clone(),
            http: reqwest::Client::new(),
        });
        client.attach_session(&session);
        session
    }

    /// Adopt whatever is in the keyring, refreshing it if stale. Returns the
    /// grant now in force, or `None` when there is nothing to resume.
    pub async fn resume(&self) -> Option<Grant> {
        let saved = Tokens::load().ok().flatten()?;
        let expired_without_refresh =
            saved.grant == Grant::User && saved.expired() && !saved.can_refresh();
        {
            let mut held = self.tokens.lock().await;
            *held = Some(saved);
        }
        match self.access_token().await {
            Ok(token) => {
                let grant = self.grant().await;
                if grant == Some(Grant::User) {
                    match self.user_token_works(&token).await {
                        Ok(true) => {}
                        Ok(false) => {
                            log::warn!(
                                "stored user token cannot access /me; discarding invalid session"
                            );
                            *self.tokens.lock().await = None;
                            self.client.clear_oauth();
                            let _ = Tokens::delete();
                            return None;
                        }
                        Err(e) => {
                            // A temporary network failure must not destroy a
                            // real login. Resource requests will retry later.
                            log::warn!("could not validate stored user session: {e}");
                        }
                    }
                }
                grant
            }
            Err(e) => {
                log::warn!("stored session unusable: {e}");
                // Pairing deployments may issue a one-hour user token without
                // a refresh token. Once it expires, forget it so startup can
                // still mint an app token and keep public playback working.
                if expired_without_refresh {
                    *self.tokens.lock().await = None;
                    self.client.clear_oauth();
                    let _ = Tokens::delete();
                }
                None
            }
        }
    }

    /// A valid access token, refreshing or re-minting as needed. Also pushes
    /// it into the API client, so callers never have to.
    pub async fn access_token(&self) -> Result<String> {
        let mut held = self.tokens.lock().await;
        if let Some(tokens) = held.as_ref().filter(|t| !t.expired()) {
            self.client.set_oauth(tokens.access_token.clone());
            return Ok(tokens.access_token.clone());
        }
        // Keep the previous pair until a refresh succeeds. A network failure
        // must not discard a user's session or downgrade it to an app token.
        let fresh = match held.as_ref() {
            Some(tokens) if tokens.can_refresh() => {
                match self.refresh(&tokens.refresh_token, tokens.grant).await {
                    Ok(new) => new,
                    // A refresh token is single-use: if it was already spent
                    // an app-only token still gets public browsing working.
                    Err(e) if tokens.grant == Grant::App => {
                        log::warn!("app token refresh failed ({e}); minting a new one");
                        self.mint_app_token().await?
                    }
                    Err(e) => return Err(e),
                }
            }
            Some(tokens) if tokens.grant == Grant::User => {
                anyhow::bail!(
                    "Your SoundCloud session expired. Sign in again in Settings → Account."
                );
            }
            Some(_) | None => self.mint_app_token().await?,
        };
        self.client.set_oauth(fresh.access_token.clone());
        let token = fresh.access_token.clone();
        let saved = fresh.save();
        *held = Some(fresh);
        saved?;
        Ok(token)
    }

    /// Which grant is in force right now.
    pub async fn grant(&self) -> Option<Grant> {
        self.tokens.lock().await.as_ref().map(|t| t.grant)
    }

    pub async fn signed_in(&self) -> bool {
        self.grant().await == Some(Grant::User)
    }

    /// Run the full browser flow and keep the resulting user tokens.
    pub async fn sign_in(&self) -> Result<()> {
        self.sign_in_notifying(|_| {}).await
    }

    pub async fn sign_in_notifying(&self, notify: impl FnOnce(String)) -> Result<()> {
        let tokens = authorize_notifying(&self.creds, notify).await?;
        anyhow::ensure!(
            self.user_token_works(&tokens.access_token).await?,
            "SoundCloud authorized the app, but the token cannot access your profile"
        );
        tokens.save()?;
        self.client.set_oauth(tokens.access_token.clone());
        let mut held = self.tokens.lock().await;
        *held = Some(tokens);
        Ok(())
    }

    /// Pairing tokens issued by api-reg look like OAuth tokens but only work
    /// for application registration. Probe `/me` before calling one a login.
    async fn user_token_works(&self, token: &str) -> Result<bool> {
        let response = self
            .http
            .get(format!("{}/me", crate::api::client::API_ROOT))
            .header(reqwest::header::ACCEPT, "application/json; charset=utf-8")
            .header(reqwest::header::AUTHORIZATION, format!("OAuth {token}"))
            .send()
            .await
            .context("validate the SoundCloud account session")?;
        if response.status().is_success() {
            return Ok(true);
        }
        if matches!(
            response.status(),
            reqwest::StatusCode::UNAUTHORIZED | reqwest::StatusCode::FORBIDDEN
        ) {
            return Ok(false);
        }
        anyhow::bail!("SoundCloud returned HTTP {} for /me", response.status())
    }

    /// Invalidate the session server-side and forget it locally.
    pub async fn sign_out(&self) -> Result<()> {
        crate::api::endpoints::sign_out(&self.client)
            .await
            .context("sign out from SoundCloud")?;
        self.tokens.lock().await.take();
        self.client.clear_oauth();
        Tokens::delete()?;
        Ok(())
    }

    /// Revoke the app grant as well as deleting this installation's tokens.
    pub async fn disconnect(&self) -> Result<()> {
        crate::api::endpoints::disconnect(&self.client)
            .await
            .context("disconnect Fastcloud from SoundCloud")?;
        self.tokens.lock().await.take();
        self.client.clear_oauth();
        Tokens::delete()?;
        Ok(())
    }

    async fn mint_app_token(&self) -> Result<Tokens> {
        client_credentials(&self.creds).await
    }

    async fn refresh(&self, refresh_token: &str, grant: Grant) -> Result<Tokens> {
        let mut refreshed = refresh(&self.creds, refresh_token).await?;
        // The response does not say which flow it came from; keep ours.
        refreshed.grant = grant;
        Ok(refreshed)
    }
}

// ===========================================================================
// Flows
// ===========================================================================

/// Authorization Code + PKCE:
///
/// 1. open the browser at `secure.soundcloud.com/authorize`,
/// 2. capture the loopback redirect,
/// 3. exchange the code (with the verifier) for tokens.
pub async fn authorize(creds: &AppCredentials) -> Result<Tokens> {
    authorize_notifying(creds, |_| {}).await
}

async fn authorize_notifying(
    creds: &AppCredentials,
    notify: impl FnOnce(String),
) -> Result<Tokens> {
    anyhow::ensure!(
        creds.redirect_is_loopback(),
        "the registered redirect URI ({}) is not a loopback address; \
         register http://127.0.0.1:{AUTH_PORT}/callback instead",
        creds.redirect_uri
    );
    let pkce = Pkce::generate();
    let state = random_state();
    let port = creds.callback_port();

    // Bound before the browser opens: a port already taken must fail here,
    // not after the user has typed their password.
    let listener = TcpListener::bind(("127.0.0.1", port))
        .with_context(|| format!("listen on 127.0.0.1:{port} for the OAuth redirect"))?;
    listener.set_nonblocking(true)?;

    let url = creds.authorize_url(&pkce, &state);
    notify(url.clone());
    tokio::task::spawn_blocking(move || webbrowser::open(&url))
        .await
        .context("start the browser task")?
        .context("open SoundCloud in your default browser")?;

    let (code, returned_state) = wait_for_callback(listener).await?;
    anyhow::ensure!(
        constant_time_eq(&returned_state, &state),
        "OAuth state mismatch: the redirect did not come from our request"
    );
    exchange_code(creds, &code, &pkce.verifier).await
}

/// Wait for the browser to hit the loopback redirect, then answer it.
async fn wait_for_callback(listener: TcpListener) -> Result<(String, String)> {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    let listener = tokio::net::TcpListener::from_std(listener)?;
    let deadline = tokio::time::Instant::now() + CALLBACK_TIMEOUT;
    loop {
        let accept = tokio::time::timeout_at(deadline, listener.accept())
            .await
            .map_err(|_| anyhow::anyhow!("timed out waiting for the browser to come back"))?;
        let (mut stream, _) = accept?;

        let mut buf = [0u8; 8192];
        let read = stream.read(&mut buf).await?;
        let request = String::from_utf8_lossy(&buf[..read]);
        let target = request
            .lines()
            .next()
            .and_then(|line| line.split(' ').nth(1))
            .unwrap_or_default();
        let params = query_params(target);

        // Browsers ask for /favicon.ico off the same origin; ignore anything
        // that is not the redirect itself and keep listening.
        if params.is_empty() && !target.starts_with("/callback") {
            let _ = stream.write_all(b"HTTP/1.1 204 No Content\r\n\r\n").await;
            continue;
        }

        let code = params.get("code").cloned();
        let state = params.get("state").cloned().unwrap_or_default();
        let error = params.get("error").cloned();
        let description = params.get("error_description").cloned();

        let body = match (&code, &error) {
            (Some(_), _) => page(
                "Signed in",
                "Fastcloud has your authorization. You can close this tab.",
            ),
            (None, Some(error)) => page(
                "Authorization failed",
                &format!(
                    "SoundCloud said: {}{}",
                    error,
                    description
                        .as_deref()
                        .map(|d| format!(" — {d}"))
                        .unwrap_or_default()
                ),
            ),
            (None, None) => page(
                "Nothing to do",
                "This request carried no authorization code.",
            ),
        };
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\n\
             Content-Length: {}\r\nConnection: close\r\n\r\n{}",
            body.len(),
            body
        );
        let _ = stream.write_all(response.as_bytes()).await;
        let _ = stream.flush().await;

        if let Some(error) = error {
            anyhow::bail!(
                "authorization denied: {error}{}",
                description.map(|d| format!(" ({d})")).unwrap_or_default()
            );
        }
        return Ok((code.context("the redirect carried no code")?, state));
    }
}

/// Exchange an authorization code for tokens.
pub async fn exchange_code(creds: &AppCredentials, code: &str, verifier: &str) -> Result<Tokens> {
    let body = post_token(
        creds,
        &[
            ("grant_type", "authorization_code"),
            ("client_id", &creds.client_id),
            ("client_secret", &creds.client_secret),
            ("redirect_uri", &creds.redirect_uri),
            ("code_verifier", verifier),
            ("code", code),
        ],
        None,
    )
    .await?;
    Tokens::from_response(&body, Grant::User)
}

/// Renew an access token. Refresh tokens are single-use, so the pair this
/// returns must be persisted before anything else uses it.
pub async fn refresh(creds: &AppCredentials, refresh_token: &str) -> Result<Tokens> {
    let body = post_token(
        creds,
        &[
            ("grant_type", "refresh_token"),
            ("client_id", &creds.client_id),
            ("client_secret", &creds.client_secret),
            ("refresh_token", refresh_token),
        ],
        None,
    )
    .await?;
    Tokens::from_response(&body, Grant::User)
}

/// App-only access for public resources. Rate-limited hard (50 per 12 h per
/// app, 30 per hour per IP), so the result is cached like any other token.
pub async fn client_credentials(creds: &AppCredentials) -> Result<Tokens> {
    let body = post_token(
        creds,
        &[("grant_type", "client_credentials")],
        Some(creds.basic_auth()),
    )
    .await?;
    Tokens::from_response(&body, Grant::App)
}

async fn post_token(
    creds: &AppCredentials,
    form: &[(&str, &str)],
    basic: Option<String>,
) -> Result<serde_json::Value> {
    let _ = creds;
    let http = reqwest::Client::new();
    let mut request = http
        .post(format!("{AUTH_ROOT}/oauth/token"))
        .header(reqwest::header::ACCEPT, "application/json; charset=utf-8")
        .form(form);
    if let Some(basic) = basic {
        request = request.header(reqwest::header::AUTHORIZATION, basic);
    }
    let response = request.send().await.context("token request")?;
    let status = response.status();
    let body: serde_json::Value = response.json().await.unwrap_or_default();
    if !status.is_success() {
        // Say what SoundCloud said, not just the code: the usual causes are a
        // redirect URI that does not match, or a spent refresh token.
        let detail = body
            .get("error_description")
            .or_else(|| body.get("error"))
            .and_then(|v| v.as_str())
            .unwrap_or("no detail");
        anyhow::bail!("token request failed ({status}): {detail}");
    }
    Ok(body)
}

/// Bring the client up: resume a session, else app-only, else nothing.
pub async fn start(client: Arc<crate::api::ApiClient>) -> Option<Arc<Session>> {
    let creds = AppCredentials::discover()?;
    let session = Session::new(creds, client);
    match session.resume().await {
        Some(grant) => log::info!("resumed a {grant:?} session"),
        None => match session.access_token().await {
            Ok(_) => log::info!("running app-only (public resources)"),
            Err(e) => {
                log::warn!("no usable token: {e}");
                return None;
            }
        },
    }
    Some(session)
}

// ===========================================================================
// Helpers
// ===========================================================================

fn page(title: &str, message: &str) -> String {
    format!(
        "<!doctype html><meta charset=utf-8><title>Fastcloud</title>\
         <body style=\"background:#121212;color:#fff;font:16px/1.5 system-ui;\
         display:grid;place-items:center;height:100vh;margin:0\">\
         <div style=\"text-align:center\"><h1 style=\"color:#f50\">{title}</h1>\
         <p>{message}</p></div>"
    )
}

/// `?a=1&b=2` from a request target, percent-decoded.
fn query_params(target: &str) -> std::collections::HashMap<String, String> {
    let mut out = std::collections::HashMap::new();
    let Some(query) = target.split('?').nth(1) else {
        return out;
    };
    for pair in query.split('&').filter(|pair| !pair.is_empty()) {
        let (key, value) = pair.split_once('=').unwrap_or((pair, ""));
        out.insert(form_decode(key), form_decode(value));
    }
    out
}

/// Percent-encode for a query value (`application/x-www-form-urlencoded`).
fn form_encode(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(byte as char)
            }
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}

fn form_decode(value: &str) -> String {
    let bytes = value.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'+' => {
                out.push(b' ');
                i += 1;
            }
            b'%' if i + 2 < bytes.len() => {
                let hex = std::str::from_utf8(&bytes[i + 1..i + 3]).unwrap_or("");
                match u8::from_str_radix(hex, 16) {
                    Ok(byte) => {
                        out.push(byte);
                        i += 3;
                    }
                    Err(_) => {
                        out.push(bytes[i]);
                        i += 1;
                    }
                }
            }
            byte => {
                out.push(byte);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn random_state() -> String {
    let mut bytes = [0u8; 16];
    rand::rng().fill_bytes(&mut bytes);
    b64(&bytes)
}

/// Compare without leaking where two strings differ.
fn constant_time_eq(a: &str, b: &str) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.bytes()
        .zip(b.bytes())
        .fold(0u8, |acc, (x, y)| acc | (x ^ y))
        == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn an_expired_user_session_is_not_silently_downgraded_or_discarded() {
        let client = Arc::new(crate::api::ApiClient::demo());
        let session = Session::new(
            AppCredentials::new("test".into(), "test".into(), None),
            client,
        );
        *session.tokens.lock().await = Some(Tokens {
            access_token: "test".into(),
            refresh_token: String::new(),
            expires_at: Some(0),
            scope: None,
            grant: Grant::User,
        });
        assert!(session.access_token().await.is_err());
        assert_eq!(session.grant().await, Some(Grant::User));
        assert!(session.access_token().await.is_err());
    }

    #[tokio::test]
    async fn concurrent_requests_reuse_a_fresh_user_token() {
        let client = Arc::new(crate::api::ApiClient::demo());
        let session = Session::new(
            AppCredentials::new("test".into(), "test".into(), None),
            client,
        );
        *session.tokens.lock().await = Some(Tokens {
            access_token: "test".into(),
            refresh_token: String::new(),
            expires_at: Some(chrono::Utc::now().timestamp() + 3600),
            scope: None,
            grant: Grant::User,
        });
        let (a, b) = tokio::join!(session.access_token(), session.access_token());
        assert_eq!(a.unwrap(), b.unwrap());
        assert_eq!(session.grant().await, Some(Grant::User));
    }

    fn creds() -> AppCredentials {
        AppCredentials::new("id-123".into(), "secret-456".into(), None)
    }

    /// OAuth 2.1 requires S256; the verifier must never be sent as the
    /// challenge (which `plain` did, and which this used to do).
    #[test]
    fn the_challenge_is_the_sha256_of_the_verifier() {
        let pkce = Pkce::generate();
        assert_eq!(pkce.verifier.len(), 43, "43 is RFC 7636's minimum");
        assert!(
            pkce.verifier
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"-._~".contains(&b))
        );
        let challenge = pkce.challenge();
        assert_ne!(challenge, pkce.verifier, "that would be `plain`");
        assert_eq!(challenge.len(), 43, "SHA-256 base64url is 43 characters");
        assert!(!challenge.contains('='), "no padding");
        // Same verifier, same challenge; different verifier, different one.
        assert_eq!(challenge, pkce.challenge());
        assert_ne!(challenge, Pkce::generate().challenge());
    }

    /// The known-answer vector from RFC 7636 appendix B.
    #[test]
    fn the_challenge_matches_the_rfc_vector() {
        let pkce = Pkce {
            verifier: "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk".into(),
        };
        assert_eq!(
            pkce.challenge(),
            "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM"
        );
    }

    #[test]
    fn the_authorize_url_carries_every_required_parameter() {
        let pkce = Pkce::generate();
        let url = creds().authorize_url(&pkce, "state-789");
        assert!(url.starts_with("https://secure.soundcloud.com/authorize?"));
        for expected in [
            "client_id=id-123",
            "response_type=code",
            "code_challenge_method=S256",
            "state=state-789",
            &format!("code_challenge={}", pkce.challenge()),
            // The redirect URI is percent-encoded, as a query value must be.
            "redirect_uri=http%3A%2F%2F127.0.0.1%3A41317%2Fcallback",
        ] {
            assert!(url.contains(expected), "{expected} missing from {url}");
        }
        // The verifier itself must never leave the process.
        assert!(!url.contains(&pkce.verifier));
    }

    /// Only HTTP Basic is accepted for the client-credentials grant.
    #[test]
    fn basic_auth_is_base64_of_id_and_secret() {
        // base64("id-123:secret-456")
        assert_eq!(creds().basic_auth(), "Basic aWQtMTIzOnNlY3JldC00NTY=");
    }

    #[test]
    fn the_callback_port_comes_from_the_redirect_uri() {
        assert_eq!(creds().callback_port(), AUTH_PORT);
        let custom = AppCredentials::new(
            "id".into(),
            "secret".into(),
            Some("http://127.0.0.1:9999/oauth".into()),
        );
        assert_eq!(custom.callback_port(), 9999);
        assert!(custom.redirect_is_loopback());
        let scheme = AppCredentials::new(
            "id".into(),
            "secret".into(),
            Some("fastcloud://callback".into()),
        );
        assert!(!scheme.redirect_is_loopback());
        // An unparsable port falls back rather than panicking.
        assert_eq!(scheme.callback_port(), AUTH_PORT);
    }

    #[test]
    fn tokens_come_out_of_a_response_with_their_grant() {
        let body = serde_json::json!({
            "access_token": "a",
            "refresh_token": "r",
            "expires_in": 3600,
            "scope": "non-expiring",
        });
        let t = Tokens::from_response(&body, Grant::User).unwrap();
        assert_eq!(t.access_token, "a");
        assert_eq!(t.refresh_token, "r");
        assert_eq!(t.scope.as_deref(), Some("non-expiring"));
        assert_eq!(t.grant, Grant::User);
        assert!(!t.expired());
        assert!(t.can_refresh());

        // An app token may come back without a refresh token.
        let app = Tokens::from_response(
            &serde_json::json!({ "access_token": "b", "expires_in": 3600 }),
            Grant::App,
        )
        .unwrap();
        assert!(!app.can_refresh());
        assert_eq!(app.grant, Grant::App);

        // A response with no token at all is an error, not an empty token.
        assert!(Tokens::from_response(&serde_json::json!({}), Grant::App).is_err());
        assert!(
            Tokens::from_response(&serde_json::json!({ "access_token": "" }), Grant::App).is_err()
        );
    }

    /// A token about to expire counts as expired, so no request starts with
    /// one that dies mid-flight.
    #[test]
    fn tokens_expire_early_by_the_refresh_skew() {
        let mut t = Tokens::from_response(
            &serde_json::json!({ "access_token": "a", "expires_in": 3600 }),
            Grant::User,
        )
        .unwrap();
        assert!(!t.expired());
        t.expires_at = Some(chrono::Utc::now().timestamp() + REFRESH_SKEW_SECS - 1);
        assert!(t.expired(), "inside the skew window");
        t.expires_at = None;
        assert!(t.expired(), "an unknown expiry is treated as expired");
    }

    #[test]
    fn redirect_parameters_are_decoded() {
        let params = query_params("/callback?code=abc%2Ddef&state=x%20y");
        assert_eq!(params.get("code").unwrap(), "abc-def");
        assert_eq!(params.get("state").unwrap(), "x y");
        // A denial comes back as an error instead of a code.
        let denied = query_params("/callback?error=access_denied&error_description=User+said+no");
        assert_eq!(denied.get("error").unwrap(), "access_denied");
        assert_eq!(denied.get("error_description").unwrap(), "User said no");
        assert!(query_params("/favicon.ico").is_empty());
    }

    #[test]
    fn form_encoding_round_trips() {
        for value in [
            "http://127.0.0.1:41317/callback",
            "a b+c",
            "%",
            "простой",
            "",
        ] {
            assert_eq!(form_decode(&form_encode(value)), value, "{value}");
        }
    }

    #[test]
    fn state_comparison_rejects_mismatches() {
        let state = random_state();
        assert!(constant_time_eq(&state, &state.clone()));
        assert!(!constant_time_eq(&state, "short"));
        assert!(!constant_time_eq("abc", "abd"));
        assert!(!constant_time_eq(&state, ""));
    }
}
