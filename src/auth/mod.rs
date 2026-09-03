#![allow(dead_code)]

use anyhow::{Context, Result};
use base64::Engine;
use rand::RngCore;
use std::net::TcpListener;

pub const AUTH_PORT: u16 = 41317;
pub const REDIRECT_URI: &str = "http://127.0.0.1:41317/callback";
pub const AUTH_SERVICE: &str = "com.fastcloud.tokens";
pub const USER_ENTRY: &str = "oauth";
pub const CLIENT_ID_ENTRY: &str = "client_id";
pub const CLIENT_SECRET_ENTRY: &str = "client_secret";

#[derive(Debug, Clone, Default)]
pub struct Pkce {
    pub verifier: String,
}

impl Pkce {
    pub fn generate() -> Self {
        let mut bytes = [0u8; 64];
        rand::rng().fill_bytes(&mut bytes);
        let verifier = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes);
        Self { verifier }
    }
}

/// Credentials used to authorize Fastcloud against SoundCloud.
#[derive(Debug, Clone)]
pub struct AppCredentials {
    pub client_id: String,
    pub client_secret: String,
}

impl AppCredentials {
    pub fn from_env() -> Option<Self> {
        let id = std::env::var("FASTCLOUD_CLIENT_ID").ok()?;
        let secret = std::env::var("FASTCLOUD_CLIENT_SECRET").ok()?;
        Some(Self {
            client_id: id,
            client_secret: secret,
        })
    }

    pub fn from_keyring() -> Option<Self> {
        let entry = keyring::Entry::new(AUTH_SERVICE, CLIENT_ID_ENTRY).ok()?;
        let id = entry.get_password().ok()?;
        let entry = keyring::Entry::new(AUTH_SERVICE, CLIENT_SECRET_ENTRY).ok()?;
        let secret = entry.get_password().ok()?;
        Some(Self {
            client_id: id,
            client_secret: secret,
        })
    }

    pub fn save_to_keyring(&self) -> Result<()> {
        let entry =
            keyring::Entry::new(AUTH_SERVICE, CLIENT_ID_ENTRY).context("create keyring entry")?;
        entry
            .set_password(&self.client_id)
            .context("store client id")?;
        let entry = keyring::Entry::new(AUTH_SERVICE, CLIENT_SECRET_ENTRY)
            .context("create keyring entry")?;
        entry
            .set_password(&self.client_secret)
            .context("store client secret")?;
        Ok(())
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Tokens {
    pub access_token: String,
    pub refresh_token: String,
    #[serde(default)]
    pub expires_at: Option<i64>,
}

impl Tokens {
    pub fn from_response(body: &serde_json::Value) -> Self {
        let now = chrono::Utc::now().timestamp();
        let expires_in = body
            .get("expires_in")
            .and_then(|v| v.as_i64())
            .unwrap_or(3600);
        Tokens {
            access_token: body
                .get("access_token")
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_string(),
            refresh_token: body
                .get("refresh_token")
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_string(),
            expires_at: Some(now + expires_in),
        }
    }

    pub fn expired(&self) -> bool {
        self.expires_at
            .map(|t| chrono::Utc::now().timestamp() >= t - 60)
            .unwrap_or(true)
    }

    // ===== Keyring persistence =====

    pub fn save(&self) -> Result<()> {
        let entry = keyring::Entry::new(AUTH_SERVICE, USER_ENTRY).context("keyring entry")?;
        let json = serde_json::to_string(self)?;
        entry.set_password(&json).context("store tokens")?;
        Ok(())
    }

    pub fn load() -> Result<Option<Self>> {
        let entry = keyring::Entry::new(AUTH_SERVICE, USER_ENTRY).context("keyring entry")?;
        match entry.get_password() {
            Ok(json) => Ok(Some(serde_json::from_str(&json)?)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(e) => Err(anyhow::anyhow!("keyring: {e}")),
        }
    }

    pub fn delete() -> Result<()> {
        let entry = keyring::Entry::new(AUTH_SERVICE, USER_ENTRY).context("keyring entry")?;
        match entry.delete_credential() {
            Ok(()) => Ok(()),
            Err(keyring::Error::NoEntry) => Ok(()),
            Err(e) => Err(anyhow::anyhow!("keyring: {e}")),
        }
    }
}

/// Run the full OAuth 2.1 authorization-code + PKCE flow.
///
/// 1. Open the browser at the SoundCloud authorize URL.
/// 2. Capture the loopback redirect at 127.0.0.1:41317/callback.
/// 3. Exchange the code for tokens.
pub async fn authorize(creds: &AppCredentials) -> Result<Tokens> {
    let pkce = Pkce::generate();
    let state = random_state();

    let listener = TcpListener::bind(("127.0.0.1", AUTH_PORT))
        .with_context(|| format!("bind 127.0.0.1:{AUTH_PORT}"))?;

    let authorize_url = format!(
        "https://secure.soundcloud.com/connect?client_id={}&response_type=code&redirect_uri={}&state={}&scope=non-expiring&code_challenge_method=plain&code_challenge={}",
        creds.client_id, REDIRECT_URI, state, pkce.verifier
    );

    let _ = webbrowser::open(&authorize_url);

    let (code, recv_state) = wait_for_callback(listener).await?;
    if recv_state != state {
        anyhow::bail!("OAuth state mismatch");
    }

    let tokens = exchange_code(creds, &code, &pkce.verifier).await?;
    tokens.save()?;
    Ok(tokens)
}

async fn wait_for_callback(listener: TcpListener) -> Result<(String, String)> {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    let (stream, _) = listener.accept()?;
    let mut stream = tokio::net::TcpStream::from_std(stream)?;
    let mut buf = [0u8; 4096];
    let n = stream.read(&mut buf).await?;
    let req = String::from_utf8_lossy(&buf[..n]).into_owned();

    let first_line = req.lines().next().unwrap_or_default();
    let url_part = first_line.split(' ').nth(1).unwrap_or("");
    let query = url_part.split('?').nth(1).unwrap_or("");

    let mut code = None;
    let mut state = None;
    for pair in query.split('&') {
        let (k, v) = pair.split_once('=').unwrap_or(("", ""));
        if k == "code" {
            code = Some(v.to_string());
        } else if k == "state" {
            state = Some(v.to_string());
        }
    }

    let body = "<html><body><h2>Fastcloud</h2><p>You can close this tab.</p><script>window.close()</script></body></html>";
    let resp = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        body.len(),
        body
    );
    stream.write_all(resp.as_bytes()).await?;
    stream.flush().await?;

    match code {
        Some(c) => Ok((c, state.unwrap_or_default())),
        None => anyhow::bail!("no code in redirect"),
    }
}

pub async fn exchange_code(creds: &AppCredentials, code: &str, verifier: &str) -> Result<Tokens> {
    let http = reqwest::Client::new();
    let resp = http
        .post("https://secure.soundcloud.com/oauth/token")
        .form(&[
            ("client_id", creds.client_id.as_str()),
            ("client_secret", creds.client_secret.as_str()),
            ("grant_type", "authorization_code"),
            ("redirect_uri", REDIRECT_URI),
            ("code", code),
            ("code_verifier", verifier),
        ])
        .send()
        .await
        .context("token exchange request")?;
    let status = resp.status();
    let body: serde_json::Value = resp.json().await.unwrap_or_default();
    if !status.is_success() {
        anyhow::bail!("token exchange failed ({status}): {body}");
    }
    Ok(Tokens::from_response(&body))
}

pub async fn refresh(creds: &AppCredentials, refresh_token: &str) -> Result<Tokens> {
    let http = reqwest::Client::new();
    let resp = http
        .post("https://secure.soundcloud.com/oauth/token")
        .form(&[
            ("client_id", creds.client_id.as_str()),
            ("client_secret", creds.client_secret.as_str()),
            ("grant_type", "refresh_token"),
            ("refresh_token", refresh_token),
        ])
        .send()
        .await
        .context("refresh request")?;
    let status = resp.status();
    let body: serde_json::Value = resp.json().await.unwrap_or_default();
    if !status.is_success() {
        anyhow::bail!("refresh failed ({status}): {body}");
    }
    Ok(Tokens::from_response(&body))
}

/// Ensure valid tokens: load from keyring, refresh proactively when expired.
pub async fn ensure_tokens(
    client: &crate::api::ApiClient,
    creds: &AppCredentials,
) -> Result<Tokens> {
    let tokens = Tokens::load()?.context("no saved tokens")?;
    let tokens = if tokens.expired() {
        let refreshed = refresh(creds, &tokens.refresh_token).await?;
        refreshed.save()?;
        refreshed
    } else {
        tokens
    };
    client.set_oauth(tokens.access_token.clone());
    Ok(tokens)
}

fn random_state() -> String {
    let mut bytes = [0u8; 16];
    rand::rng().fill_bytes(&mut bytes);
    hex(&bytes)
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pkce_plain_roundtrip() {
        let p = Pkce::generate();
        assert!(!p.verifier.is_empty());
        assert_eq!(p.verifier.len(), 86);
        // Plain challenge == verifier per SoundCloud PKCE flow
    }

    #[test]
    fn tokens_from_response() {
        let body = serde_json::json!({
            "access_token": "a",
            "refresh_token": "r",
            "expires_in": 3600,
        });
        let t = Tokens::from_response(&body);
        assert_eq!(t.access_token, "a");
        assert_eq!(t.refresh_token, "r");
        assert!(!t.expired());
    }
}
