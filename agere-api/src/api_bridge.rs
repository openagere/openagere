use crate::TransportError;
use crate::error::ApiError;
use crate::rate_limits::parse_promo_message;
use crate::rate_limits::parse_rate_limit_for_limit;
use agere_protocol::auth::PlanType;
use agere_protocol::error::AgereErr;
use agere_protocol::error::RateLimitedError;
use agere_protocol::error::RetryLimitReachedError;
use agere_protocol::error::UnexpectedResponseError;
use agere_protocol::error::UsageLimitReachedError;
use base64::Engine;
use chrono::DateTime;
use chrono::Utc;
use http::HeaderMap;
use serde::Deserialize;
use serde_json::Value;

pub fn map_api_error(err: ApiError) -> AgereErr {
    match err {
        ApiError::ContextWindowExceeded => AgereErr::ContextWindowExceeded,
        ApiError::QuotaExceeded => AgereErr::QuotaExceeded,
        ApiError::UsageNotIncluded => AgereErr::UsageNotIncluded,
        ApiError::Retryable { message, delay } => AgereErr::Stream(message, delay),
        ApiError::Stream(msg) => AgereErr::Stream(msg, None),
        ApiError::ServerOverloaded => AgereErr::ServerOverloaded,
        ApiError::Api { status, message } => AgereErr::UnexpectedStatus(UnexpectedResponseError {
            status,
            body: message,
            url: None,
            cf_ray: None,
            request_id: None,
            identity_authorization_error: None,
            identity_error_code: None,
        }),
        ApiError::InvalidRequest { message } => AgereErr::InvalidRequest(message),
        ApiError::CyberPolicy { message } => AgereErr::CyberPolicy { message },
        ApiError::Transport(transport) => match transport {
            TransportError::Http {
                status,
                url,
                headers,
                body,
            } => {
                let body_text = body.unwrap_or_default();

                if status == http::StatusCode::SERVICE_UNAVAILABLE
                    && let Ok(value) = serde_json::from_str::<serde_json::Value>(&body_text)
                    && matches!(
                        value
                            .get("error")
                            .and_then(|error| error.get("code"))
                            .and_then(serde_json::Value::as_str),
                        Some("server_is_overloaded" | "slow_down")
                    )
                {
                    return AgereErr::ServerOverloaded;
                }

                if status == http::StatusCode::BAD_REQUEST {
                    if let Ok(parsed) = serde_json::from_str::<Value>(&body_text)
                        && let Some(error) = parsed.get("error")
                        && error.get("code").and_then(Value::as_str)
                            == Some(CYBER_POLICY_ERROR_CODE)
                    {
                        let message = error
                            .get("message")
                            .and_then(Value::as_str)
                            .filter(|message| !message.trim().is_empty())
                            .map(str::to_string)
                            .unwrap_or_else(|| CYBER_POLICY_FALLBACK_MESSAGE.to_string());
                        AgereErr::CyberPolicy { message }
                    } else if body_text
                        .contains("The image data you provided does not represent a valid image")
                    {
                        AgereErr::InvalidImageRequest()
                    } else {
                        AgereErr::InvalidRequest(body_text)
                    }
                } else if status == http::StatusCode::INTERNAL_SERVER_ERROR {
                    AgereErr::InternalServerError
                } else if status == http::StatusCode::TOO_MANY_REQUESTS {
                    if let Ok(err) = serde_json::from_str::<UsageErrorResponse>(&body_text) {
                        if err.error.error_type.as_deref() == Some("usage_limit_reached") {
                            let limit_id = extract_header(headers.as_ref(), ACTIVE_LIMIT_HEADER);
                            let rate_limits = headers.as_ref().and_then(|map| {
                                parse_rate_limit_for_limit(map, limit_id.as_deref())
                            });
                            let promo_message = headers.as_ref().and_then(parse_promo_message);
                            let resets_at = err
                                .error
                                .resets_at
                                .and_then(|seconds| DateTime::<Utc>::from_timestamp(seconds, 0));
                            return AgereErr::UsageLimitReached(UsageLimitReachedError {
                                plan_type: err.error.plan_type,
                                resets_at,
                                rate_limits: rate_limits.map(Box::new),
                                promo_message,
                            });
                        } else if err.error.error_type.as_deref() == Some("usage_not_included") {
                            return AgereErr::UsageNotIncluded;
                        }
                    }

                    let parsed_rate_limit =
                        serde_json::from_str::<RateLimitErrorResponse>(&body_text).ok();
                    let error_code = parsed_rate_limit.as_ref().and_then(rate_limit_error_code);
                    if error_code == Some("insufficient_quota") {
                        return AgereErr::QuotaExceeded;
                    }
                    if error_code == Some("usage_not_included") {
                        return AgereErr::UsageNotIncluded;
                    }
                    if !is_recoverable_rate_limit_error_code(
                        error_code,
                        RECOVERABLE_RATE_LIMIT_ERROR_CODES,
                    ) {
                        return AgereErr::UnexpectedStatus(UnexpectedResponseError {
                            status,
                            body: body_text,
                            url,
                            cf_ray: extract_header(headers.as_ref(), CF_RAY_HEADER),
                            request_id: extract_request_id(headers.as_ref()),
                            identity_authorization_error: extract_header(
                                headers.as_ref(),
                                X_OPENAI_AUTHORIZATION_ERROR_HEADER,
                            ),
                            identity_error_code: extract_x_error_json_code(headers.as_ref()),
                        });
                    }
                    let retry_after = headers.as_ref().and_then(parse_retry_after_header);
                    let resets_at = parsed_rate_limit
                        .as_ref()
                        .and_then(|err| err.error.resets_at)
                        .and_then(|seconds| DateTime::<Utc>::from_timestamp(seconds, 0))
                        .or_else(|| headers.as_ref().and_then(parse_rate_limit_resets_at));
                    let message = parsed_rate_limit
                        .and_then(|err| err.error.message)
                        .filter(|message| !message.trim().is_empty())
                        .unwrap_or_else(|| {
                            let trimmed_body = body_text.trim();
                            if trimmed_body.is_empty() {
                                "rate limited".to_string()
                            } else {
                                trimmed_body.to_string()
                            }
                        });
                    AgereErr::RateLimited(RateLimitedError {
                        status,
                        message,
                        retry_after,
                        resets_at,
                        request_id: extract_request_tracking_id(headers.as_ref()),
                    })
                } else {
                    AgereErr::UnexpectedStatus(UnexpectedResponseError {
                        status,
                        body: body_text,
                        url,
                        cf_ray: extract_header(headers.as_ref(), CF_RAY_HEADER),
                        request_id: extract_request_id(headers.as_ref()),
                        identity_authorization_error: extract_header(
                            headers.as_ref(),
                            X_OPENAI_AUTHORIZATION_ERROR_HEADER,
                        ),
                        identity_error_code: extract_x_error_json_code(headers.as_ref()),
                    })
                }
            }
            TransportError::RetryLimit => AgereErr::RetryLimit(RetryLimitReachedError {
                status: http::StatusCode::INTERNAL_SERVER_ERROR,
                request_id: None,
            }),
            TransportError::Timeout => AgereErr::Timeout,
            TransportError::Network(msg) | TransportError::Build(msg) => {
                AgereErr::Stream(msg, None)
            }
        },
        ApiError::RateLimit(msg) => AgereErr::RateLimited(RateLimitedError {
            status: http::StatusCode::TOO_MANY_REQUESTS,
            message: msg,
            retry_after: None,
            resets_at: None,
            request_id: None,
        }),
        ApiError::RateLimited {
            status,
            message,
            retry_after,
            resets_at,
            request_id,
        } => AgereErr::RateLimited(RateLimitedError {
            status,
            message,
            retry_after,
            resets_at,
            request_id,
        }),
    }
}

const ACTIVE_LIMIT_HEADER: &str = "x-agere-active-limit";
const REQUEST_ID_HEADER: &str = "x-request-id";
const OAI_REQUEST_ID_HEADER: &str = "x-oai-request-id";
const CF_RAY_HEADER: &str = "cf-ray";
const X_OPENAI_AUTHORIZATION_ERROR_HEADER: &str = "x-openai-authorization-error";
const X_ERROR_JSON_HEADER: &str = "x-error-json";
const CYBER_POLICY_ERROR_CODE: &str = "cyber_policy";
const CYBER_POLICY_FALLBACK_MESSAGE: &str =
    "This request has been flagged for possible cybersecurity risk.";
const RECOVERABLE_RATE_LIMIT_ERROR_CODES: &[&str] = &[];
const MAX_RETRY_AFTER: std::time::Duration = std::time::Duration::from_secs(60 * 60 * 24);

#[cfg(test)]
#[path = "api_bridge_tests.rs"]
mod tests;

fn extract_request_tracking_id(headers: Option<&HeaderMap>) -> Option<String> {
    extract_request_id(headers).or_else(|| extract_header(headers, CF_RAY_HEADER))
}

fn extract_request_id(headers: Option<&HeaderMap>) -> Option<String> {
    extract_header(headers, REQUEST_ID_HEADER)
        .or_else(|| extract_header(headers, OAI_REQUEST_ID_HEADER))
}

fn extract_header(headers: Option<&HeaderMap>, name: &str) -> Option<String> {
    headers.and_then(|map| {
        map.get(name)
            .and_then(|value| value.to_str().ok())
            .map(str::to_string)
    })
}

/// Parse the standard HTTP `Retry-After` header. Supports both the
/// `<delta-seconds>` and `<HTTP-date>` forms; values that cannot be parsed
/// (or that resolve to non-positive durations) are ignored.
fn parse_retry_after_header(headers: &HeaderMap) -> Option<std::time::Duration> {
    let raw = headers
        .get(http::header::RETRY_AFTER)?
        .to_str()
        .ok()?
        .trim();
    if raw.is_empty() {
        return None;
    }
    if let Ok(secs) = raw.parse::<u64>() {
        return retry_after_from_secs(secs);
    }
    if let Ok(seconds) = raw.parse::<f64>()
        && seconds.is_finite()
        && seconds > 0.0
    {
        if seconds >= MAX_RETRY_AFTER.as_secs_f64() {
            return Some(MAX_RETRY_AFTER);
        }
        return Some(std::time::Duration::from_secs_f64(seconds));
    }
    let parsed = DateTime::parse_from_rfc2822(raw)
        .map(|dt| dt.with_timezone(&Utc))
        .or_else(|_| DateTime::parse_from_rfc3339(raw).map(|dt| dt.with_timezone(&Utc)))
        .ok()?;
    let delta = parsed.signed_duration_since(Utc::now()).num_seconds();
    (delta > 0)
        .then(|| u64::try_from(delta).ok())
        .flatten()
        .and_then(retry_after_from_secs)
}

fn retry_after_from_secs(secs: u64) -> Option<std::time::Duration> {
    if secs == 0 {
        return None;
    }
    Some(std::time::Duration::from_secs(
        secs.min(MAX_RETRY_AFTER.as_secs()),
    ))
}

/// Read the soonest reset hint from the rate-limit header families.
///
/// Agere reset headers use absolute Unix seconds. Standard rate-limit reset
/// headers use relative seconds until reset.
fn parse_rate_limit_resets_at(headers: &HeaderMap) -> Option<DateTime<Utc>> {
    const ABSOLUTE_HEADERS: &[&str] = &["x-agere-primary-reset-at", "x-agere-secondary-reset-at"];
    const RELATIVE_HEADERS: &[&str] = &[
        "x-ratelimit-reset",
        "x-ratelimit-reset-requests",
        "x-ratelimit-reset-tokens",
    ];

    let absolute_resets = ABSOLUTE_HEADERS.iter().filter_map(|name| {
        let raw = headers.get(*name)?.to_str().ok()?.trim();
        let seconds = raw.parse::<i64>().ok()?;
        DateTime::<Utc>::from_timestamp(seconds, 0)
    });
    let relative_resets = RELATIVE_HEADERS.iter().filter_map(|name| {
        let raw = headers.get(*name)?.to_str().ok()?.trim();
        let seconds = raw.parse::<i64>().ok()?;
        (seconds > 0).then(|| Utc::now() + chrono::Duration::seconds(seconds))
    });

    absolute_resets.chain(relative_resets).min()
}

fn rate_limit_error_code(response: &RateLimitErrorResponse) -> Option<&str> {
    response
        .error
        .code
        .as_deref()
        .or(response.error.error_type.as_deref())
}

fn is_recoverable_rate_limit_error_code(code: Option<&str>, allowlist: &[&str]) -> bool {
    allowlist.is_empty() || code.is_some_and(|code| allowlist.contains(&code))
}

fn extract_x_error_json_code(headers: Option<&HeaderMap>) -> Option<String> {
    let encoded = extract_header(headers, X_ERROR_JSON_HEADER)?;
    let decoded = base64::engine::general_purpose::STANDARD
        .decode(encoded)
        .ok()?;
    let parsed = serde_json::from_slice::<Value>(&decoded).ok()?;
    parsed
        .get("error")
        .and_then(|error| error.get("code"))
        .and_then(Value::as_str)
        .map(str::to_string)
}

#[derive(Debug, Deserialize)]
struct UsageErrorResponse {
    error: UsageErrorBody,
}

#[derive(Debug, Deserialize)]
struct UsageErrorBody {
    #[serde(rename = "type")]
    error_type: Option<String>,
    plan_type: Option<PlanType>,
    resets_at: Option<i64>,
}

#[derive(Debug, Deserialize)]
struct RateLimitErrorResponse {
    error: RateLimitErrorBody,
}

#[derive(Debug, Deserialize)]
struct RateLimitErrorBody {
    #[serde(rename = "type")]
    error_type: Option<String>,
    code: Option<String>,
    message: Option<String>,
    resets_at: Option<i64>,
}
