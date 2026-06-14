use agere_api::ApiError;
use regex_lite::Regex;
use std::sync::OnceLock;
use std::time::Duration;

/// Map an Anthropic SSE `error` event to a codex `ApiError`.
pub(crate) fn map_anthropic_error(error_type: &str, message: &str) -> ApiError {
    match error_type {
        "overloaded_error" => ApiError::ServerOverloaded,
        "rate_limit_error" => {
            let delay = parse_anthropic_retry_after(message);
            ApiError::Retryable {
                message: message.to_string(),
                delay,
            }
        }
        "invalid_request_error" => ApiError::InvalidRequest {
            message: message.to_string(),
        },
        "authentication_error" => ApiError::Stream(format!("authentication error: {message}")),
        "permission_error" => ApiError::Stream(format!("permission error: {message}")),
        "not_found_error" => ApiError::Stream(format!("not found: {message}")),
        _ => ApiError::Stream(format!("api error ({error_type}): {message}")),
    }
}

fn parse_anthropic_retry_after(message: &str) -> Option<Duration> {
    let re = retry_after_regex();
    if let Some(captures) = re.captures(message) {
        let value = captures.get(1)?.as_str().parse::<f64>().ok()?;
        Some(Duration::from_secs_f64(value))
    } else {
        None
    }
}

fn retry_after_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"in\s*(\d+(?:\.\d+)?)\s*s")
            .unwrap_or_else(|_| Regex::new(r"\d+(?:\.\d+)?").unwrap_or_else(|_| unreachable!()))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use assert_matches::assert_matches;

    #[test]
    fn map_overloaded_error() {
        let result = map_anthropic_error("overloaded_error", "Server overloaded");
        assert_matches!(result, ApiError::ServerOverloaded);
    }

    #[test]
    fn map_rate_limit_error_with_delay() {
        let result = map_anthropic_error("rate_limit_error", "Try again in 5s");
        assert_matches!(
            result,
            ApiError::Retryable { message, delay: Some(_) }
                if message.contains("Try again in 5s")
        );
    }

    #[test]
    fn map_invalid_request_error() {
        let result = map_anthropic_error("invalid_request_error", "Bad prompt");
        assert_matches!(result, ApiError::InvalidRequest { message } if message == "Bad prompt");
    }

    #[test]
    fn map_authentication_error() {
        let result = map_anthropic_error("authentication_error", "Invalid key");
        assert_matches!(result, ApiError::Stream(msg)
            if msg.contains("authentication error") && msg.contains("Invalid key"));
    }

    #[test]
    fn map_permission_error() {
        let result = map_anthropic_error("permission_error", "No access");
        assert_matches!(result, ApiError::Stream(msg)
            if msg.contains("permission error"));
    }

    #[test]
    fn map_not_found_error() {
        let result = map_anthropic_error("not_found_error", "Not found");
        assert_matches!(result, ApiError::Stream(msg)
            if msg.contains("not found"));
    }

    #[test]
    fn map_unknown_error_type() {
        let result = map_anthropic_error("unknown_type", "Something happened");
        assert_matches!(result, ApiError::Stream(msg)
            if msg.contains("unknown_type") && msg.contains("Something happened"));
    }
}
