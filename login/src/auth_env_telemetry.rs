use agere_model_provider_info::ModelProviderInfo;
use agere_otel::AuthEnvTelemetryMetadata;

use crate::AGERE_API_KEY_ENV_VAR;
use crate::OPENAI_API_KEY_ENV_VAR;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AuthEnvTelemetry {
    pub openai_api_key_env_present: bool,
    pub agere_api_key_env_present: bool,
    pub agere_api_key_env_enabled: bool,
    pub provider_env_key_name: Option<String>,
    pub provider_env_key_present: Option<bool>,
}

impl AuthEnvTelemetry {
    pub fn to_otel_metadata(&self) -> AuthEnvTelemetryMetadata {
        AuthEnvTelemetryMetadata {
            openai_api_key_env_present: self.openai_api_key_env_present,
            agere_api_key_env_present: self.agere_api_key_env_present,
            agere_api_key_env_enabled: self.agere_api_key_env_enabled,
            provider_env_key_name: self.provider_env_key_name.clone(),
            provider_env_key_present: self.provider_env_key_present,
        }
    }
}

pub fn collect_auth_env_telemetry(
    provider: &ModelProviderInfo,
    agere_api_key_env_enabled: bool,
) -> AuthEnvTelemetry {
    AuthEnvTelemetry {
        openai_api_key_env_present: env_var_present(OPENAI_API_KEY_ENV_VAR),
        agere_api_key_env_present: env_var_present(AGERE_API_KEY_ENV_VAR),
        agere_api_key_env_enabled,
        provider_env_key_name: provider.env_key.as_ref().map(|_| "configured".to_string()),
        provider_env_key_present: provider.env_key.as_deref().map(env_var_present),
    }
}

fn env_var_present(name: &str) -> bool {
    match std::env::var(name) {
        Ok(value) => !value.trim().is_empty(),
        Err(std::env::VarError::NotUnicode(_)) => true,
        Err(std::env::VarError::NotPresent) => false,
    }
}
