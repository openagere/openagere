use agere_api::ApiError;
use http::StatusCode;

#[allow(dead_code)]
pub(crate) fn map_chat_error(status: u16, message: &str) -> ApiError {
    match status {
        400 => ApiError::InvalidRequest {
            message: message.to_string(),
        },
        401 => ApiError::Api {
            status: StatusCode::UNAUTHORIZED,
            message: message.to_string(),
        },
        403 => ApiError::CyberPolicy {
            message: message.to_string(),
        },
        429 => ApiError::RateLimit(message.to_string()),
        500..=599 => ApiError::Api {
            status: StatusCode::from_u16(status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR),
            message: message.to_string(),
        },
        _ => ApiError::Stream(format!("HTTP {status}: {message}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn map_400_to_invalid_request() {
        assert!(matches!(
            map_chat_error(400, "invalid_request"),
            ApiError::InvalidRequest { .. }
        ));
    }

    #[test]
    fn map_401_to_api_unauthorized() {
        let err = map_chat_error(401, "invalid_api_key");
        assert!(matches!(err, ApiError::Api { status, .. } if status == StatusCode::UNAUTHORIZED));
    }

    #[test]
    fn map_429_to_rate_limit() {
        assert!(matches!(
            map_chat_error(429, "rate_limit_exceeded"),
            ApiError::RateLimit(_)
        ));
    }

    #[test]
    fn map_500_to_api_internal() {
        let err = map_chat_error(500, "server_error");
        assert!(
            matches!(err, ApiError::Api { status, .. } if status == StatusCode::INTERNAL_SERVER_ERROR)
        );
    }

    #[test]
    fn map_unknown_code_to_stream() {
        assert!(matches!(map_chat_error(418, "teapot"), ApiError::Stream(_)));
    }
}
