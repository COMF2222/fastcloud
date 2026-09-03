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
    pub fn status(&self) -> Option<u16> {
        match self {
            ApiError::Http { status, .. } => Some(*status),
            _ => None,
        }
    }
}
