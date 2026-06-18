use crate::rate_limits::RateLimitError;
use agere_client::TransportError;
use chrono::DateTime;
use chrono::Utc;
use http::StatusCode;
use std::time::Duration;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ApiError {
    #[error(transparent)]
    Transport(#[from] TransportError),
    #[error("api error {status}: {message}")]
    Api { status: StatusCode, message: String },
    #[error("stream error: {0}")]
    Stream(String),
    #[error("context window exceeded")]
    ContextWindowExceeded,
    #[error("quota exceeded")]
    QuotaExceeded,
    #[error("usage not included")]
    UsageNotIncluded,
    #[error("retryable error: {message}")]
    Retryable {
        message: String,
        delay: Option<Duration>,
    },
    #[error("rate limit: {0}")]
    RateLimit(String),
    /// HTTP 429 response that does not represent a per-account usage limit.
    /// Carries any retry hints surfaced by the provider so the turn loop can
    /// schedule the slow-retry policy without losing context.
    #[error("rate limited (status {status}): {message}")]
    RateLimited {
        status: StatusCode,
        message: String,
        retry_after: Option<Duration>,
        resets_at: Option<DateTime<Utc>>,
        request_id: Option<String>,
    },
    #[error("invalid request: {message}")]
    InvalidRequest { message: String },
    #[error("cyber policy: {message}")]
    CyberPolicy { message: String },
    #[error("server overloaded")]
    ServerOverloaded,
}

impl From<RateLimitError> for ApiError {
    fn from(err: RateLimitError) -> Self {
        Self::RateLimit(err.to_string())
    }
}
