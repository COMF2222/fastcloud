//! Registering the user's own SoundCloud application, without them leaving the
//! app.
//!
//! SoundCloud requires a `client_secret` for every token exchange (see the
//! module docs in [`super`]), so Fastcloud cannot ship one: each user needs an
//! application of their own. Sending them to `developers.soundcloud.com` to
//! copy two strings back is the obvious way and a bad one — so this does what
//! SoundCloud's own `sc-api-auth` CLI does instead:
//!
//! 1. **Pair a device.** `POST /pairing/codes` returns a six-character code and
//!    a poll token. The user opens `secure.soundcloud.com/activate/<code>` and
//!    signs in there.
//! 2. **Poll until they have.** `GET /pairing/codes/<code>` reports `created`
//!    until the page is used, then `activated`.
//! 3. **Take a token.** `POST /pairing/sign-in` exchanges the activated code
//!    for an access token for the *registration* API.
//! 4. **Register.** `POST /me/apps` creates the application and answers with
//!    the user's own `client_id` and `client_secret`.
//!
//! One sign-in therefore both proves the account has Artist Pro — SoundCloud
//! requires it to register an app, and says so in the 403 — and produces the
//! keys. Nothing is typed and nothing is copied.
//!
//! ## Why not the loopback flow
//!
//! [`super::authorize`] needs a redirect URI registered character for
//! character, and a free port to listen on. Neither exists before the user has
//! an application, and the device-code flow needs neither: no listener, no
//! port, no redirect. That is also why a firewall prompt cannot block sign-up.
//!
//! ## What this depends on
//!
//! `api-reg.soundcloud.com` is not in SoundCloud's published OpenAPI spec; it
//! is what their CLI uses, and [`PAIRING_CLIENT_ID`] is that CLI's own public
//! client. It works, and it is the documented path to credentials — but it can
//! change without notice, which is why [`super::AppCredentials::save_to_keyring`]
//! and the Settings dialog keep a manual route open.

use anyhow::{Context, Result};
use serde::Deserialize;

/// The registration API. A different host from both the resource API and the
/// token endpoint.
const REGISTER_ROOT: &str = "https://api-reg.soundcloud.com";

/// Where the user activates a pairing code.
const ACTIVATE_ROOT: &str = "https://secure.soundcloud.com/activate";

/// SoundCloud's own public client for the credentials CLI, which is the only
/// published client that can reach the pairing endpoints. Public by design: the
/// pairing flow carries no secret.
pub const PAIRING_CLIENT_ID: &str = "nXIZT4VQQYkgHs75vpIYbnINQciCkV5Y";

/// How long to wait for the user to finish signing in.
const ACTIVATION_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10 * 60);

/// The floor on the poll interval, whatever the server suggests: polling faster
/// than this is rude and gets rate-limited.
const MIN_POLL: std::time::Duration = std::time::Duration::from_secs(1);
const DEFAULT_POLL: std::time::Duration = std::time::Duration::from_secs(3);

/// What Fastcloud registers itself as. SoundCloud shows these on the user's own
/// apps page, and their terms ask for them to be accurate.
const APP_NAME: &str = "Fastcloud";
const APP_DESCRIPTION: &str =
    "Fastcloud, a native desktop SoundCloud client: browsing, search and playback.";
const APP_WEBSITE: &str = "https://github.com/COMF2222/fastcloud";

/// Why a registration attempt could not produce credentials.
///
/// Separated from a plain error string because the interface has to say
/// something different for each: "you need Artist Pro" is not a bug report, and
/// "you already have an app" is not even a failure.
#[derive(Debug, thiserror::Error)]
pub enum RegisterError {
    /// The account cannot register an application. SoundCloud requires an
    /// Artist Pro subscription for this and says so here.
    #[error("{0}")]
    NeedsArtistPro(String),
    /// SoundCloud refused for some other reason it named.
    #[error("SoundCloud refused: {0}")]
    Refused(String),
    /// The user never finished the browser sign-in.
    #[error("timed out waiting for the SoundCloud sign-in")]
    TimedOut,
    /// The pairing code expired before it was used.
    #[error("the sign-in code expired; start again")]
    CodeExpired,
    /// The user asked to stop.
    #[error("sign-in cancelled")]
    Cancelled,
    /// Anything else: network, an unexpected shape, a changed endpoint.
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

/// A started pairing: what to show the user, and what to poll with.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pairing {
    /// The six-character code, which is also part of [`Self::url`].
    pub code: String,
    /// The page the user opens to sign in.
    pub url: String,
    /// Proves this process started the pairing, so only it can claim the token.
    poll_token: String,
    /// How often the server asked to be polled.
    interval: std::time::Duration,
}

impl Pairing {
    /// The page to open in a browser.
    pub fn url(&self) -> &str {
        &self.url
    }
}

/// Start a pairing. The user must then open [`Pairing::url`].
pub async fn begin(http: &reqwest::Client) -> Result<Pairing, RegisterError> {
    #[derive(Deserialize)]
    struct Response {
        code: String,
        poll_token: String,
        #[serde(default)]
        poll_interval_seconds: Option<serde_json::Value>,
    }

    let body = pairing_body();
    let response = http
        .post(format!("{REGISTER_ROOT}/pairing/codes"))
        .query(&[("client_id", PAIRING_CLIENT_ID)])
        .header(reqwest::header::ACCEPT, "application/json; charset=utf-8")
        .json(&body)
        .send()
        .await
        .context("ask SoundCloud for a sign-in code")?;
    let status = response.status();
    let text = response.text().await.unwrap_or_default();
    if !status.is_success() {
        return Err(classify(status, &text));
    }
    let parsed: Response =
        serde_json::from_str(&text).context("SoundCloud's sign-in code response")?;
    Ok(Pairing {
        url: activate_url(&parsed.code),
        code: parsed.code,
        poll_token: parsed.poll_token,
        interval: parsed
            .poll_interval_seconds
            .as_ref()
            .and_then(parse_poll_interval)
            .map(std::time::Duration::from_secs)
            .unwrap_or(DEFAULT_POLL)
            .max(MIN_POLL),
    })
}

/// Keep the registration request identical to SoundCloud's own credential
/// helper. The service currently identifies this public pairing client as a
/// CLI device even when the caller has a desktop interface.
fn pairing_body() -> serde_json::Value {
    serde_json::json!({
        "device": {
            "id": uuid_v4(),
            "type": "cli",
            "name": device_name(),
        }
    })
}

/// The registration API has returned this field as both a JSON number and a
/// string. SoundCloud's reference client deliberately accepts either.
fn parse_poll_interval(value: &serde_json::Value) -> Option<u64> {
    value
        .as_u64()
        .or_else(|| value.as_str().and_then(|seconds| seconds.parse().ok()))
}

/// Wait for the user to activate the code, then register an application and
/// return its credentials.
///
/// `cancel` ends the wait early: the user closed the dialog rather than the
/// browser tab.
pub async fn finish(
    http: &reqwest::Client,
    pairing: &Pairing,
    mut cancel: tokio::sync::watch::Receiver<bool>,
) -> Result<Registered, RegisterError> {
    let deadline = tokio::time::Instant::now() + ACTIVATION_TIMEOUT;
    loop {
        if *cancel.borrow_and_update() {
            return Err(RegisterError::Cancelled);
        }
        match poll(http, pairing).await? {
            Activation::Activated => break,
            Activation::Waiting => {}
            Activation::Expired => return Err(RegisterError::CodeExpired),
        }
        // Whichever comes first: the next poll, the user giving up, or the
        // ten-minute deadline.
        tokio::select! {
            _ = tokio::time::sleep(pairing.interval) => {}
            _ = cancel.changed() => {
                if *cancel.borrow() {
                    return Err(RegisterError::Cancelled);
                }
            }
            _ = tokio::time::sleep_until(deadline) => return Err(RegisterError::TimedOut),
        }
        if tokio::time::Instant::now() >= deadline {
            return Err(RegisterError::TimedOut);
        }
    }
    let token = sign_in(http, pairing).await?;
    register(http, &token).await
}

/// Where a pairing code stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Activation {
    Waiting,
    Activated,
    Expired,
}

async fn poll(http: &reqwest::Client, pairing: &Pairing) -> Result<Activation, RegisterError> {
    #[derive(Deserialize)]
    struct Response {
        #[serde(default)]
        status: Option<String>,
    }

    let response = http
        .get(format!(
            "{REGISTER_ROOT}/pairing/codes/{}",
            super::form_encode(&pairing.code)
        ))
        .query(&[
            ("client_id", PAIRING_CLIENT_ID),
            ("poll_token", pairing.poll_token.as_str()),
        ])
        .header(reqwest::header::ACCEPT, "application/json; charset=utf-8")
        .send()
        .await
        .context("check whether the sign-in finished")?;
    let status = response.status();
    // The code is not visible to the pairing service the instant it is made;
    // a 404 here means "not yet", not "gone".
    if status == reqwest::StatusCode::NOT_FOUND {
        return Ok(Activation::Waiting);
    }
    let text = response.text().await.unwrap_or_default();
    if !status.is_success() {
        return Err(classify(status, &text));
    }
    let parsed: Response = serde_json::from_str(&text).unwrap_or(Response { status: None });
    Ok(activation_of(parsed.status.as_deref()))
}

/// Read a pairing status. Anything unrecognised counts as "keep waiting",
/// because a new status word must not look like a failure.
fn activation_of(status: Option<&str>) -> Activation {
    match status {
        Some("activated") => Activation::Activated,
        Some("expired") => Activation::Expired,
        _ => Activation::Waiting,
    }
}

/// Exchange an activated pairing code for an access token.
async fn sign_in(http: &reqwest::Client, pairing: &Pairing) -> Result<String, RegisterError> {
    #[derive(Deserialize)]
    struct Session {
        access_token: Option<String>,
    }
    #[derive(Deserialize)]
    struct Response {
        session: Option<Session>,
        access_token: Option<String>,
    }
    let body = serde_json::json!({
        "client_id": PAIRING_CLIENT_ID,
        "pairing_code": pairing.code,
        "poll_token": pairing.poll_token,
        "scope": "",
    });
    let response = http
        .post(format!("{REGISTER_ROOT}/pairing/sign-in"))
        .query(&[("client_id", PAIRING_CLIENT_ID)])
        .header(reqwest::header::ACCEPT, "application/json; charset=utf-8")
        .json(&body)
        .send()
        .await
        .context("finish the SoundCloud sign-in")?;
    let status = response.status();
    let text = response.text().await.unwrap_or_default();
    if status == reqwest::StatusCode::NOT_FOUND {
        return Err(RegisterError::CodeExpired);
    }
    if !status.is_success() {
        return Err(classify(status, &text));
    }
    let parsed: Response = serde_json::from_str(&text).context("the sign-in response")?;
    // This token belongs to api-reg.soundcloud.com. It authorizes creating or
    // reading the developer application, but the resource API rejects it for
    // /me, likes and playlists. Never persist it as the listener's session.
    parsed
        .session
        .and_then(|session| session.access_token)
        .or(parsed.access_token)
        .filter(|token| !token.is_empty())
        .ok_or_else(|| RegisterError::Other(anyhow::anyhow!("sign-in returned no access token")))
}

/// The credentials a registration produced.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Registered {
    pub credentials: super::AppCredentials,
    /// Whether this app already existed, rather than being created now.
    /// Registering twice is not an error: SoundCloud allows one app per
    /// account and hands the existing one back.
    pub existing: bool,
}

/// Create the application, or fetch the one this account already has.
async fn register(http: &reqwest::Client, token: &str) -> Result<Registered, RegisterError> {
    let body = serde_json::json!({
        "name": APP_NAME,
        "description": APP_DESCRIPTION,
        "website": APP_WEBSITE,
    });
    let response = http
        .post(format!("{REGISTER_ROOT}/me/apps"))
        .header(reqwest::header::ACCEPT, "application/json; charset=utf-8")
        // The registration API takes the same `OAuth` scheme as the resource
        // API, not `Bearer`.
        .header(reqwest::header::AUTHORIZATION, format!("OAuth {token}"))
        .json(&body)
        .send()
        .await
        .context("register the application")?;
    let status = response.status();
    let text = response.text().await.unwrap_or_default();
    if status.is_success() {
        return Ok(Registered {
            credentials: credentials_from(&serde_json::from_str(&text).unwrap_or_default())?,
            existing: false,
        });
    }
    // One app per account: the existing one is what we wanted anyway.
    if api_error_code(&text).as_deref() == Some("user_already_has_application") {
        return Ok(Registered {
            credentials: existing_app(http, token).await?,
            existing: true,
        });
    }
    Err(classify(status, &text))
}

/// The account's existing application, for when registration says there is one.
async fn existing_app(
    http: &reqwest::Client,
    token: &str,
) -> Result<super::AppCredentials, RegisterError> {
    let response = http
        .get(format!("{REGISTER_ROOT}/me/apps"))
        .header(reqwest::header::ACCEPT, "application/json; charset=utf-8")
        .header(reqwest::header::AUTHORIZATION, format!("OAuth {token}"))
        .send()
        .await
        .context("read the account's applications")?;
    let status = response.status();
    let text = response.text().await.unwrap_or_default();
    if !status.is_success() {
        return Err(classify(status, &text));
    }
    let page: serde_json::Value = serde_json::from_str(&text).unwrap_or_default();
    let app = page
        .get("collection")
        .and_then(|c| c.as_array())
        .and_then(|apps| {
            apps.iter()
                .find(|app| app.get("client_id").and_then(|v| v.as_str()).is_some())
        })
        .ok_or_else(|| {
            RegisterError::Other(anyhow::anyhow!(
                "SoundCloud says this account has an application but did not return it"
            ))
        })?;
    credentials_from(app)
}

/// Pull a client id and secret out of an application object.
fn credentials_from(app: &serde_json::Value) -> Result<super::AppCredentials, RegisterError> {
    let field = |name: &str| {
        app.get(name)
            .and_then(|v| v.as_str())
            .filter(|value| !value.is_empty())
            .map(str::to_owned)
    };
    let client_id = field("client_id").ok_or_else(|| {
        RegisterError::Other(anyhow::anyhow!("the registration carried no client id"))
    })?;
    let client_secret = field("client_secret").ok_or_else(|| {
        // Without a secret there is no token exchange at all, so this is fatal
        // rather than something to paper over with an empty string.
        RegisterError::Other(anyhow::anyhow!(
            "the registration carried no client secret, which SoundCloud requires for tokens"
        ))
    })?;
    // Whatever redirect URI the app was registered with wins: signing in later
    // has to match it character for character, and ours is only the default.
    Ok(super::AppCredentials::new(
        client_id,
        client_secret,
        field("redirect_uri"),
    ))
}

/// The `code` of the first error in a JSON:API-ish error body.
fn api_error_code(text: &str) -> Option<String> {
    serde_json::from_str::<serde_json::Value>(text)
        .ok()?
        .get("errors")?
        .as_array()?
        .first()?
        .get("code")?
        .as_str()
        .map(str::to_owned)
}

/// The human-readable message from an error body, if it has one.
fn api_error_message(text: &str) -> Option<String> {
    let body: serde_json::Value = serde_json::from_str(text).ok()?;
    let first = body.get("errors")?.as_array()?.first()?;
    first
        .get("error_message")
        .or_else(|| first.get("message"))?
        .as_str()
        .filter(|message| !message.is_empty())
        .map(str::to_owned)
}

/// Turn a failed response into the error the interface should show.
fn classify(status: reqwest::StatusCode, text: &str) -> RegisterError {
    let code = api_error_code(text);
    let message = api_error_message(text);
    match code.as_deref() {
        // The one refusal that is a fact about the account rather than a
        // problem: SoundCloud requires Artist Pro to register an app.
        Some("application_creation_not_available") => {
            RegisterError::NeedsArtistPro(message.unwrap_or_else(|| ARTIST_PRO_MESSAGE.to_owned()))
        }
        Some(other) => {
            RegisterError::Refused(message.unwrap_or_else(|| format!("{other} ({status})")))
        }
        // A 403 with no code at all is the same refusal in practice: nothing
        // else about this flow is forbidden once the user has signed in.
        None if status == reqwest::StatusCode::FORBIDDEN => {
            RegisterError::NeedsArtistPro(message.unwrap_or_else(|| ARTIST_PRO_MESSAGE.to_owned()))
        }
        None => RegisterError::Refused(message.unwrap_or_else(|| format!("HTTP {status}"))),
    }
}

/// What to say when the account cannot register an app and SoundCloud did not
/// word it themselves.
pub const ARTIST_PRO_MESSAGE: &str = "SoundCloud only issues API keys to accounts with an Artist Pro subscription. \
     That is their requirement for the API, not ours.";

/// A name for the paired device, so the user can recognise it in their
/// SoundCloud settings.
fn device_name() -> String {
    match hostname() {
        Some(host) => format!("Fastcloud on {host}"),
        None => "Fastcloud".to_owned(),
    }
}

fn hostname() -> Option<String> {
    // No crate for this: the variable every platform sets is enough, and a
    // missing one just means a plainer label.
    for key in ["COMPUTERNAME", "HOSTNAME", "HOST"] {
        if let Ok(value) = std::env::var(key) {
            let value = value.trim().to_owned();
            if !value.is_empty() {
                return Some(value);
            }
        }
    }
    None
}

/// A random version-4 UUID, which is what the pairing API wants for a device
/// id. Written out rather than pulling in a crate for sixteen bytes.
fn uuid_v4() -> String {
    use rand::RngCore;
    let mut bytes = [0u8; 16];
    rand::rng().fill_bytes(&mut bytes);
    // Version 4, variant 1, per RFC 4122.
    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    let hex: String = bytes.iter().map(|byte| format!("{byte:02x}")).collect();
    format!(
        "{}-{}-{}-{}-{}",
        &hex[0..8],
        &hex[8..12],
        &hex[12..16],
        &hex[16..20],
        &hex[20..32]
    )
}

fn activate_url(code: &str) -> String {
    format!("{ACTIVATE_ROOT}/{}", super::form_encode(code))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A version-4 UUID has to be shaped like one, and two must differ.
    #[test]
    fn device_ids_are_version_four_uuids() {
        let id = uuid_v4();
        assert_eq!(id.len(), 36);
        let parts: Vec<&str> = id.split('-').collect();
        assert_eq!(
            parts.iter().map(|p| p.len()).collect::<Vec<_>>(),
            [8, 4, 4, 4, 12]
        );
        assert!(id.chars().all(|c| c.is_ascii_hexdigit() || c == '-'));
        // The version nibble and the variant bits are what make it a v4.
        assert_eq!(parts[2].as_bytes()[0], b'4', "not version 4");
        assert!(
            matches!(parts[3].as_bytes()[0], b'8' | b'9' | b'a' | b'b'),
            "wrong variant"
        );
        assert_ne!(uuid_v4(), uuid_v4());
    }

    /// The code goes into the URL the user opens, escaped: it comes from the
    /// server, so it is not ours to trust.
    #[test]
    fn the_activation_url_carries_the_code() {
        assert_eq!(
            activate_url("3UZU59"),
            "https://secure.soundcloud.com/activate/3UZU59"
        );
        assert!(
            !activate_url("a/b").contains("a/b"),
            "a slash escaped the path"
        );
    }

    /// Match SoundCloud's maintained `sc-api-auth` request. The pairing API
    /// recognises this public client as a CLI even when our UI is graphical.
    #[test]
    fn pairing_request_matches_soundclouds_reference_client() {
        let body = pairing_body();
        let device = &body["device"];
        assert_eq!(device["type"], "cli");
        assert!(device["id"].as_str().is_some_and(|id| id.len() == 36));
        assert!(device["name"].as_str().is_some_and(|name| !name.is_empty()));
    }

    /// SoundCloud's reference client coerces this field to a string before
    /// parsing because deployments have returned both JSON representations.
    #[test]
    fn poll_interval_accepts_numbers_and_strings() {
        assert_eq!(parse_poll_interval(&serde_json::json!(4)), Some(4));
        assert_eq!(parse_poll_interval(&serde_json::json!("5")), Some(5));
        assert_eq!(parse_poll_interval(&serde_json::json!("later")), None);
        assert_eq!(parse_poll_interval(&serde_json::Value::Null), None);
    }

    /// An unknown status must read as "keep waiting", or a new status word
    /// SoundCloud adds would fail every sign-in.
    #[test]
    fn only_the_known_statuses_end_the_wait() {
        assert_eq!(activation_of(Some("activated")), Activation::Activated);
        assert_eq!(activation_of(Some("expired")), Activation::Expired);
        assert_eq!(activation_of(Some("created")), Activation::Waiting);
        assert_eq!(activation_of(Some("something_new")), Activation::Waiting);
        assert_eq!(activation_of(None), Activation::Waiting);
    }

    /// The refusal that means "you need Artist Pro" has to be told apart from
    /// every other one, because it is the only one where the answer is a
    /// subscription rather than a retry.
    #[test]
    fn artist_pro_is_recognised_from_the_error_code() {
        let body = r#"{"errors":[{"code":"application_creation_not_available",
            "error_message":"You need Artist Pro."}]}"#;
        match classify(reqwest::StatusCode::FORBIDDEN, body) {
            RegisterError::NeedsArtistPro(message) => {
                assert_eq!(message, "You need Artist Pro.", "SoundCloud's own wording");
            }
            other => panic!("classified as {other:?}"),
        }
        // …and with no message of their own, ours stands in.
        let bare = r#"{"errors":[{"code":"application_creation_not_available"}]}"#;
        match classify(reqwest::StatusCode::FORBIDDEN, bare) {
            RegisterError::NeedsArtistPro(message) => assert_eq!(message, ARTIST_PRO_MESSAGE),
            other => panic!("classified as {other:?}"),
        }
        // A 403 with no code at all: nothing else here is forbidden after a
        // sign-in, so it is the same story.
        match classify(reqwest::StatusCode::FORBIDDEN, "") {
            RegisterError::NeedsArtistPro(message) => assert_eq!(message, ARTIST_PRO_MESSAGE),
            other => panic!("classified as {other:?}"),
        }
    }

    /// Every other refusal keeps SoundCloud's own words, because guessing at
    /// what a named error meant is how a wrong message gets shown.
    #[test]
    fn other_refusals_are_reported_as_they_came() {
        let body = r#"{"errors":[{"code":"application_name_not_allowed",
            "error_message":"This application name is not allowed."}]}"#;
        match classify(reqwest::StatusCode::FORBIDDEN, body) {
            RegisterError::Refused(message) => {
                assert_eq!(message, "This application name is not allowed.");
            }
            other => panic!("classified as {other:?}"),
        }
        // No body at all still says something specific enough to act on.
        match classify(reqwest::StatusCode::BAD_GATEWAY, "") {
            RegisterError::Refused(message) => assert!(message.contains("502")),
            other => panic!("classified as {other:?}"),
        }
        // A named code with no message falls back to the code, not to nothing.
        let coded = r#"{"errors":[{"code":"some_new_code"}]}"#;
        match classify(reqwest::StatusCode::BAD_REQUEST, coded) {
            RegisterError::Refused(message) => assert!(message.contains("some_new_code")),
            other => panic!("classified as {other:?}"),
        }
    }

    #[test]
    fn error_bodies_are_read_leniently() {
        assert_eq!(
            api_error_code(r#"{"errors":[{"code":"x"}]}"#).as_deref(),
            Some("x")
        );
        // Every shape that is not that must be `None` rather than a panic.
        for body in [
            "",
            "null",
            "[]",
            "{}",
            r#"{"errors":[]}"#,
            r#"{"errors":{}}"#,
        ] {
            assert_eq!(api_error_code(body), None, "{body}");
            assert_eq!(api_error_message(body), None, "{body}");
        }
        // `message` is accepted as well as `error_message`.
        assert_eq!(
            api_error_message(r#"{"errors":[{"message":"m"}]}"#).as_deref(),
            Some("m")
        );
        // An empty message is no message, so ours stands in instead.
        assert_eq!(
            api_error_message(r#"{"errors":[{"error_message":""}]}"#),
            None
        );
    }

    /// A registration without a secret is useless — SoundCloud needs one for
    /// every token — so it has to fail here rather than at the first request.
    #[test]
    fn credentials_need_both_halves() {
        let app = serde_json::json!({
            "client_id": "id-1",
            "client_secret": "secret-1",
            "name": "Fastcloud",
        });
        let creds = credentials_from(&app).expect("a complete app");
        assert_eq!(creds.client_id, "id-1");
        assert_eq!(creds.client_secret, "secret-1");
        // No redirect URI registered: ours is the default.
        assert_eq!(creds.redirect_uri, crate::auth::DEFAULT_REDIRECT_URI);

        // A registered redirect URI wins, because sign-in must match it.
        let with_redirect = serde_json::json!({
            "client_id": "id-1",
            "client_secret": "secret-1",
            "redirect_uri": "http://127.0.0.1:9000/cb",
        });
        assert_eq!(
            credentials_from(&with_redirect).unwrap().redirect_uri,
            "http://127.0.0.1:9000/cb"
        );

        for missing in [
            serde_json::json!({ "client_secret": "s" }),
            serde_json::json!({ "client_id": "i" }),
            serde_json::json!({ "client_id": "", "client_secret": "s" }),
            serde_json::json!({ "client_id": "i", "client_secret": "" }),
            serde_json::json!({}),
        ] {
            assert!(
                credentials_from(&missing).is_err(),
                "accepted {missing} as credentials"
            );
        }
    }

    /// The pairing client is SoundCloud's own published one, and the hosts are
    /// theirs: a typo here would send a sign-in somewhere else entirely.
    #[test]
    fn the_endpoints_are_soundclouds() {
        assert_eq!(PAIRING_CLIENT_ID.len(), 32);
        assert!(PAIRING_CLIENT_ID.chars().all(|c| c.is_ascii_alphanumeric()));
        assert!(REGISTER_ROOT.starts_with("https://"));
        assert!(REGISTER_ROOT.ends_with(".soundcloud.com"));
        assert!(ACTIVATE_ROOT.starts_with("https://secure.soundcloud.com/"));
        // The registration API is a different host from the resource API and
        // the token endpoint; conflating them is the classic mistake here.
        assert_ne!(REGISTER_ROOT, crate::api::client::API_ROOT);
        assert_ne!(REGISTER_ROOT, crate::auth::AUTH_ROOT);
    }

    /// The device label reaches the user's SoundCloud settings, so it has to
    /// name this app whatever the environment looks like.
    #[test]
    fn the_device_is_named_after_this_app() {
        assert!(device_name().starts_with("Fastcloud"));
        assert!(!device_name().ends_with(' '));
    }
}
