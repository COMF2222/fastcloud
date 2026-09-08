#![allow(dead_code)]

use thiserror::Error;

pub type Result<T> = std::result::Result<T, ApiError>;

#[derive(Debug, Error)]
pub enum ApiError {
    #[error("network error: {0}")]
    Network(#[from] reqwest::Error),

    #[error("rate limited (HTTP 429), retry after {retry_after_ms:?}")]
    RateLimited { retry_after_ms: Option<u64> },

    #[error("unauthorized: token expired or revoked (HTTP 401)")]
    Unauthorized,

    #[error("HTTP {status}: {body}")]
    Http { status: u16, body: String },

    #[error("bad request: {0}")]
    Bad(String),

    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),

    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

impl ApiError {
    pub fn user_message(&self) -> String {
        match self {
            Self::Unauthorized => {
                "Your SoundCloud session expired. Sign in again in Settings → Account.".into()
            }
            Self::Http { status: 404, .. } => {
                "This item is no longer available on SoundCloud.".into()
            }
            Self::Http { status: 403, .. } => {
                "SoundCloud does not allow access to this item with this account.".into()
            }
            Self::Http {
                status: 500..=599, ..
            } => "SoundCloud is temporarily unavailable. Please try again shortly.".into(),
            Self::Network(_) => {
                "Could not reach SoundCloud. Check your connection and try again.".into()
            }
            Self::RateLimited { .. } => {
                "SoundCloud's request limit was reached. Waiting before retrying.".into()
            }
            Self::Json(_) => {
                "SoundCloud returned data that could not be read. Please retry.".into()
            }
            _ => self.to_string(),
        }
    }

    pub fn status(&self) -> Option<u16> {
        match self {
            ApiError::Http { status, .. } => Some(*status),
            _ => None,
        }
    }
}
