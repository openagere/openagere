//! Helpers powering the slow-retry policy applied when the model provider
//! returns HTTP `429 Too Many Requests`.
//!
//! The schedule itself lives in `agere_config::types::RateLimitRetryConfig`.
//! This module turns a [`RateLimitedError`] plus the configured policy into a
//! concrete [`Duration`] to sleep, honouring server-supplied hints when they
//! are present and the user opted into them via `respect_resets_at`.

use std::time::Duration;

use agere_config::types::RateLimitRetryConfig;
use agere_protocol::error::RateLimitedError;
use chrono::Utc;
use tokio_util::sync::CancellationToken;

/// Compute the wait duration before the n-th slow rate-limit retry attempt.
///
/// `attempt` is 0-based: pass `0` for the very first slow retry. The result is
/// always clamped by `cfg.cap_secs` so a misbehaving server cannot strand a
/// turn for an unbounded amount of time.
pub(crate) fn compute_rate_limit_delay(
    cfg: &RateLimitRetryConfig,
    err: &RateLimitedError,
    attempt: u32,
) -> Duration {
    let scheduled = Duration::from_secs(cfg.delay_secs_for_attempt(attempt));
    let cap = Duration::from_secs(cfg.cap_secs);
    if cfg.respect_resets_at {
        let hint = err
            .resets_at
            .and_then(|reset_at| (reset_at - Utc::now()).to_std().ok())
            .or(err.retry_after);
        if let Some(hint) = hint {
            return hint.min(cap);
        }
    }
    scheduled.min(cap)
}

pub(crate) fn truncate_rate_limit_reason(message: &str) -> String {
    let trimmed = message.trim();
    const MAX: usize = 240;
    if trimmed.len() <= MAX {
        return trimmed.to_string();
    }

    let mut end = MAX;
    while !trimmed.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}...", &trimmed[..end])
}

pub(crate) async fn sleep_until_rate_limit_retry(
    delay: Duration,
    cancellation_token: &CancellationToken,
) -> bool {
    tokio::select! {
        () = tokio::time::sleep(delay) => true,
        () = cancellation_token.cancelled() => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration as ChronoDuration;
    use http::StatusCode;
    use pretty_assertions::assert_eq;

    fn err_with_resets_in(seconds: i64) -> RateLimitedError {
        RateLimitedError {
            status: StatusCode::TOO_MANY_REQUESTS,
            message: "rate limited".to_string(),
            retry_after: None,
            resets_at: Some(Utc::now() + ChronoDuration::seconds(seconds)),
            request_id: None,
        }
    }

    fn err_with_retry_after(secs: u64) -> RateLimitedError {
        RateLimitedError {
            status: StatusCode::TOO_MANY_REQUESTS,
            message: "rate limited".to_string(),
            retry_after: Some(Duration::from_secs(secs)),
            resets_at: None,
            request_id: None,
        }
    }

    #[test]
    fn default_schedule_is_capped_at_ten_minutes() {
        let cfg = RateLimitRetryConfig::default();
        let err = err_with_retry_after(0);

        assert_eq!(
            compute_rate_limit_delay(&cfg, &err, 0),
            Duration::from_secs(60),
        );
        assert_eq!(
            compute_rate_limit_delay(&cfg, &err, 1),
            Duration::from_secs(120),
        );
        assert_eq!(
            compute_rate_limit_delay(&cfg, &err, 2),
            Duration::from_secs(300),
        );
        assert_eq!(
            compute_rate_limit_delay(&cfg, &err, 3),
            Duration::from_secs(600),
        );
        assert_eq!(
            compute_rate_limit_delay(&cfg, &err, 50),
            Duration::from_secs(600),
        );
    }

    #[test]
    fn ignores_reset_hints_by_default() {
        let cfg = RateLimitRetryConfig::default();
        let err = err_with_resets_in(18_000);

        let delay = compute_rate_limit_delay(&cfg, &err, 0);

        assert_eq!(delay, Duration::from_secs(60));
    }

    #[test]
    fn caps_reset_hints_when_enabled() {
        let cfg = RateLimitRetryConfig {
            respect_resets_at: true,
            ..RateLimitRetryConfig::default()
        };
        let err = err_with_resets_in(18_000);

        let delay = compute_rate_limit_delay(&cfg, &err, 0);

        assert_eq!(delay, Duration::from_secs(600));
    }

    #[test]
    fn uses_retry_after_when_enabled() {
        let cfg = RateLimitRetryConfig {
            respect_resets_at: true,
            ..RateLimitRetryConfig::default()
        };
        let err = err_with_retry_after(180);

        let delay = compute_rate_limit_delay(&cfg, &err, 0);

        assert_eq!(delay, Duration::from_secs(180));
    }

    #[test]
    fn caps_retry_after_when_enabled() {
        let cfg = RateLimitRetryConfig {
            respect_resets_at: true,
            ..RateLimitRetryConfig::default()
        };
        let err = err_with_retry_after(900);

        let delay = compute_rate_limit_delay(&cfg, &err, 0);

        assert_eq!(delay, Duration::from_secs(600));
    }

    #[test]
    fn last_entry_used_after_schedule_exhausted() {
        let cfg = RateLimitRetryConfig {
            delays_secs: vec![10, 20, 30],
            cap_secs: 600,
            ..RateLimitRetryConfig::default()
        };
        let err = err_with_retry_after(0);
        assert_eq!(
            compute_rate_limit_delay(&cfg, &err, 0),
            Duration::from_secs(10),
        );
        assert_eq!(
            compute_rate_limit_delay(&cfg, &err, 5),
            Duration::from_secs(30),
        );
    }

    #[test]
    fn truncates_reason_at_utf8_char_boundary() {
        let reason = format!("{}界tail", "a".repeat(239));

        assert_eq!(
            truncate_rate_limit_reason(&reason),
            format!("{}...", "a".repeat(239)),
        );
    }

    #[tokio::test]
    async fn retry_sleep_completes_when_delay_elapses() {
        let cancellation_token = CancellationToken::new();
        let completed = tokio::time::timeout(
            Duration::from_secs(1),
            sleep_until_rate_limit_retry(Duration::from_millis(1), &cancellation_token),
        )
        .await
        .expect("short sleep should complete");

        assert!(completed);
    }

    #[tokio::test]
    async fn retry_sleep_stops_when_cancelled() {
        let cancellation_token = CancellationToken::new();
        cancellation_token.cancel();

        let completed = tokio::time::timeout(
            Duration::from_secs(1),
            sleep_until_rate_limit_retry(Duration::from_secs(600), &cancellation_token),
        )
        .await
        .expect("cancelled sleep should complete");

        assert!(!completed);
    }
}
