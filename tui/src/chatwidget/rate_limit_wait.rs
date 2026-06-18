//! TUI helpers for rendering the live countdown surfaced by
//! [`agere_protocol::protocol::RateLimitWaitingEvent`].
//!
//! The state object is intentionally tiny: the [`ChatWidget`] only stores a
//! [`RateLimitWaitState`] while a slow-retry sleep is in flight, and clears
//! it on stream success/failure so the regular status indicator returns.

use agere_protocol::protocol::RateLimitWaitingEvent;

/// Snapshot describing an in-flight rate-limit retry sleep. Constructed when
/// the core emits a [`RateLimitWaitingEvent`] and ticked every second so the
/// countdown stays accurate without requiring extra protocol round-trips.
#[derive(Debug, Clone)]
pub(crate) struct RateLimitWaitState {
    pub attempt: u32,
    pub max_attempts: u32,
    pub resume_at_unix_seconds: i64,
    pub initial_wait_seconds: u64,
    pub reason: String,
}

impl RateLimitWaitState {
    pub(crate) fn from_event(event: RateLimitWaitingEvent) -> Self {
        Self {
            attempt: event.attempt,
            max_attempts: event.max_attempts,
            resume_at_unix_seconds: event.resume_at_unix_seconds,
            initial_wait_seconds: event.wait_seconds,
            reason: event.reason,
        }
    }

    /// Seconds remaining until the retry fires. Returns `0` once we've
    /// reached or passed `resume_at`.
    pub(crate) fn remaining_seconds(&self, now_unix_seconds: i64) -> u64 {
        let delta = self.resume_at_unix_seconds.saturating_sub(now_unix_seconds);
        delta.max(0) as u64
    }

    /// Header line used by the status indicator while we're waiting. Mirrors
    /// the structure of stream-error retries so users get a familiar shape.
    pub(crate) fn header(&self, now_unix_seconds: i64) -> String {
        let remaining = self.remaining_seconds(now_unix_seconds);
        let countdown = format_duration(remaining);
        let attempt = self.attempt.max(1);
        let max_attempts = self.max_attempts;
        if max_attempts == 0 {
            format!("Rate limited - retrying in {countdown} (attempt {attempt})")
        } else {
            format!("Rate limited - retrying in {countdown} (attempt {attempt}/{max_attempts})",)
        }
    }

    /// Detail string surfaced under the header. Includes the reason text and
    /// the static schedule the user configured (or the defaults).
    pub(crate) fn details(&self) -> Option<String> {
        let trimmed = self.reason.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(format!(
                "Sleeping {} (server reason: {trimmed})",
                format_duration(self.initial_wait_seconds),
            ))
        }
    }
}

/// Format a whole-second duration as `Hh Mm Ss` / `Mm Ss` / `Ss`.
fn format_duration(total_secs: u64) -> String {
    let hours = total_secs / 3_600;
    let minutes = (total_secs % 3_600) / 60;
    let seconds = total_secs % 60;
    if hours > 0 {
        format!("{hours}h {minutes:02}m {seconds:02}s")
    } else if minutes > 0 {
        format!("{minutes}m {seconds:02}s")
    } else {
        format!("{seconds}s")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    fn sample_event() -> RateLimitWaitingEvent {
        RateLimitWaitingEvent {
            attempt: 2,
            max_attempts: 5,
            resume_at_unix_seconds: 1_000,
            wait_seconds: 65,
            reason: "rate limited (status 429): too many requests".to_string(),
        }
    }

    #[test]
    fn header_with_max_attempts() {
        let state = RateLimitWaitState::from_event(sample_event());
        assert_eq!(
            state.header(940),
            "Rate limited - retrying in 1m 00s (attempt 2/5)",
        );
    }

    #[test]
    fn header_without_max_attempts() {
        let mut event = sample_event();
        event.max_attempts = 0;
        let state = RateLimitWaitState::from_event(event);
        assert_eq!(
            state.header(990),
            "Rate limited - retrying in 10s (attempt 2)",
        );
    }

    #[test]
    fn header_clamps_negative_remaining() {
        let state = RateLimitWaitState::from_event(sample_event());
        assert_eq!(
            state.header(2_000),
            "Rate limited - retrying in 0s (attempt 2/5)",
        );
    }

    #[test]
    fn formats_hour_durations() {
        assert_eq!(format_duration(3_725), "1h 02m 05s");
    }

    #[test]
    fn details_includes_reason() {
        let state = RateLimitWaitState::from_event(sample_event());
        assert_eq!(
            state.details(),
            Some(
                "Sleeping 1m 05s (server reason: rate limited (status 429): too many requests)"
                    .to_string()
            ),
        );
    }
}
