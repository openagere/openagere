/// API version header value for the Anthropic Messages API.
pub(crate) const ANTHROPIC_API_VERSION: &str = "2023-06-01";

/// Default max_tokens to use when not specified by caller.
/// Streaming is always enabled, so we give the model room to think and act.
pub const DEFAULT_MAX_TOKENS: u32 = 64000;

/// Default beta features to request.
pub(crate) const DEFAULT_BETA_FEATURES: &[&str] = &["prompt-caching-2024-07-31"];

/// Build the `anthropic-beta` header value.
pub(crate) fn beta_header(features: &[String]) -> String {
    if features.is_empty() {
        DEFAULT_BETA_FEATURES.join(",")
    } else {
        features.join(",")
    }
}
