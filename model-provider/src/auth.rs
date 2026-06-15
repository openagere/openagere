use std::sync::Arc;

use agere_api::AuthProvider;
use agere_api::SharedAuthProvider;
use agere_login::AgereAuth;
use agere_model_provider_info::ModelProviderInfo;
use http::HeaderMap;

use crate::bearer_auth_provider::BearerAuthProvider;

// Some providers are meant to send no auth headers. Examples include local OSS
// providers and custom test providers with `requires_provider_auth = false`.
#[derive(Clone, Debug)]
struct UnauthenticatedAuthProvider;

impl AuthProvider for UnauthenticatedAuthProvider {
    fn add_auth_headers(&self, _headers: &mut HeaderMap) {}
}

pub fn unauthenticated_auth_provider() -> SharedAuthProvider {
    Arc::new(UnauthenticatedAuthProvider)
}

/// Returns the auth manager for the provider (always the base manager in simplified auth).
pub(crate) fn auth_manager_for_provider(
    auth_manager: Option<Arc<agere_login::AuthManager>>,
    _provider: &ModelProviderInfo,
) -> Option<Arc<agere_login::AuthManager>> {
    auth_manager
}

pub(crate) fn resolve_provider_auth(
    auth: Option<&AgereAuth>,
    provider: &ModelProviderInfo,
) -> agere_protocol::error::Result<SharedAuthProvider> {
    if let Some(auth) = bearer_auth_for_provider(provider)? {
        return Ok(Arc::new(auth));
    }

    Ok(match auth {
        Some(auth) => auth_provider_from_auth(auth),
        None => unauthenticated_auth_provider(),
    })
}

fn bearer_auth_for_provider(
    provider: &ModelProviderInfo,
) -> agere_protocol::error::Result<Option<BearerAuthProvider>> {
    // Priority: experimental_bearer_token > env_key environment variable
    // This allows provider.toml's api_key to work even if env_key is configured.
    if let Some(ref token) = provider.experimental_bearer_token {
        if !token.trim().is_empty() {
            return Ok(Some(BearerAuthProvider::new(token.clone())));
        }
    }

    // Try env_key environment variable only if experimental_bearer_token is not set.
    if let Some(api_key) = provider.api_key()? {
        return Ok(Some(BearerAuthProvider::new(api_key)));
    }

    Ok(None)
}

/// Builds requests-header auth for an API key auth snapshot.
pub fn auth_provider_from_auth(auth: &AgereAuth) -> SharedAuthProvider {
    Arc::new(BearerAuthProvider {
        token: auth.get_token().ok(),
        account_id: auth.get_account_id(),
        is_fedramp_account: auth.is_fedramp_account(),
    })
}

#[cfg(test)]
mod tests {
    use agere_model_provider_info::WireApi;
    use agere_model_provider_info::create_oss_provider_with_base_url;

    use super::*;

    #[test]
    fn unauthenticated_auth_provider_adds_no_headers() {
        let provider =
            create_oss_provider_with_base_url("http://localhost:11434/v1", WireApi::Responses);
        let auth = resolve_provider_auth(/*auth*/ None, &provider).expect("auth should resolve");

        assert!(auth.to_auth_headers().is_empty());
    }
}
