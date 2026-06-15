use std::path::PathBuf;
use std::sync::Arc;

use agere_api::Provider;
use agere_api::SharedAuthProvider;
use agere_login::AgereAuth;
use agere_login::AuthManager;
use agere_model_provider_info::ModelProviderInfo;
use agere_models_manager::collaboration_mode_presets::CollaborationModesConfig;
use agere_models_manager::manager::SharedModelsManager;
use agere_models_manager::manager::StaticModelsManager;
use agere_protocol::account::ProviderAccount;
use agere_protocol::error::Result;
use agere_protocol::openai_models::ModelsResponse;

use crate::provider::ModelProvider;
use crate::provider::ProviderAccountResult;
use crate::provider::ProviderAccountState;
use crate::provider::ProviderCapabilities;

/// Resolve API key: env var (env_key) > experimental_bearer_token from config.
fn resolve_api_key(info: &ModelProviderInfo) -> Result<String> {
    // 1. Try env_key as environment variable name
    if let Some(env_key) = info.env_key.as_deref()
        && let Ok(val) = std::env::var(env_key)
        && !val.is_empty()
    {
        return Ok(val);
    }

    // 2. Try experimental_bearer_token (direct value in config)
    if let Some(token) = info.experimental_bearer_token.as_deref()
        && !token.is_empty()
    {
        return Ok(token.to_string());
    }

    // 3. No key found
    let key_name = info.env_key.as_deref().unwrap_or("(not set)");
    Err(agere_protocol::error::AgereErr::EnvVar(
        agere_protocol::error::EnvVarError {
            var: key_name.to_string(),
            instructions: Some(format!(
                "Set the environment variable '{key_name}', \
                 or set `api_key` in your config for provider '{}'.",
                info.wire_api
            )),
        },
    ))
}

/// Runtime provider for Anthropic Messages API compatible backends.
#[derive(Clone, Debug)]
pub(crate) struct AnthropicModelProvider {
    pub(crate) info: ModelProviderInfo,
    config_models: Vec<agere_config::config_toml::ModelConfig>,
}

impl AnthropicModelProvider {
    pub(crate) fn new(
        provider_info: ModelProviderInfo,
        config_models: Vec<agere_config::config_toml::ModelConfig>,
    ) -> Self {
        Self {
            info: provider_info,
            config_models,
        }
    }
}

#[async_trait::async_trait]
impl ModelProvider for AnthropicModelProvider {
    fn info(&self) -> &ModelProviderInfo {
        &self.info
    }

    fn capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities {
            namespace_tools: false,
            image_generation: false,
            web_search: false,
        }
    }

    fn auth_manager(&self) -> Option<Arc<AuthManager>> {
        None
    }

    async fn auth(&self) -> Option<AgereAuth> {
        None
    }

    fn account_state(&self) -> ProviderAccountResult {
        Ok(ProviderAccountState {
            account: Some(ProviderAccount::ApiKey),
            requires_provider_auth: false,
        })
    }

    async fn api_provider(&self) -> Result<Provider> {
        self.info.to_api_provider(/*auth_mode*/ None)
    }

    async fn api_auth(&self) -> Result<SharedAuthProvider> {
        use crate::auth::auth_provider_from_auth;

        // Resolve API key: env var > experimental_bearer_token
        let token = resolve_api_key(&self.info)?;
        let auth = AgereAuth::from_api_key(&token);
        Ok(auth_provider_from_auth(&auth))
    }

    fn models_manager(
        &self,
        _agere_home: PathBuf,
        config_model_catalog: Option<ModelsResponse>,
        collaboration_modes_config: CollaborationModesConfig,
    ) -> SharedModelsManager {
        let catalog = config_model_catalog.or_else(|| {
            if self.config_models.is_empty() {
                // Fall back to bundled models.json for online logic
                agere_models_manager::bundled_models_response().ok()
            } else {
                Some(crate::model_catalog::build_models_response(
                    &self.config_models,
                    self.info.wire_api,
                ))
            }
        });
        Arc::new(StaticModelsManager::new(
            /*auth_manager*/ None,
            catalog.unwrap_or_else(|| ModelsResponse { models: vec![] }),
            collaboration_modes_config,
        ))
    }
}

#[cfg(test)]
mod tests {
    use agere_model_provider_info::WireApi;
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn capabilities_disable_unsupported_features() {
        let provider = AnthropicModelProvider::new(
            ModelProviderInfo {
                base_url: Some("https://api.anthropic.com".into()),
                env_key: Some("ANTHROPIC_API_KEY".into()),
                wire_api: WireApi::Anthropic,
                ..Default::default()
            },
            vec![],
        );

        assert_eq!(
            provider.capabilities(),
            ProviderCapabilities {
                namespace_tools: false,
                image_generation: false,
                web_search: false,
            }
        );
    }

    #[tokio::test]
    async fn models_manager_uses_config_models() {
        use agere_model_provider_info::WireApi;

        let config_models = vec![agere_config::config_toml::ModelConfig {
            name: "test-model".to_string(),
            context_window: Some(100_000),
        }];
        let provider = AnthropicModelProvider::new(
            ModelProviderInfo {
                wire_api: WireApi::Anthropic,
                env_key: Some("DEEPSEEK_API_KEY".into()),
                ..Default::default()
            },
            config_models,
        );

        let mgr = provider.models_manager(PathBuf::new(), None, Default::default());

        let catalog = mgr
            .raw_model_catalog(agere_models_manager::manager::RefreshStrategy::Online)
            .await;
        assert!(catalog.models.iter().any(|m| m.slug == "test-model"));
        assert!(
            !catalog
                .models
                .iter()
                .any(|m| m.used_fallback_model_metadata)
        );
    }
}
