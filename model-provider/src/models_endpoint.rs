use std::sync::Arc;
use std::time::Duration;

use agere_api::ApiError;
use agere_api::ModelsClient;
use agere_api::RequestTelemetry;
use agere_api::ReqwestTransport;
use agere_api::TransportError;
use agere_api::auth_header_telemetry;
use agere_api::map_api_error;
use agere_feedback::FeedbackRequestTags;
use agere_feedback::emit_feedback_request_tags_with_auth_env;
use agere_login::AgereAuth;
use agere_login::AuthEnvTelemetry;
use agere_login::AuthManager;
use agere_login::collect_auth_env_telemetry;
use agere_login::default_client::build_reqwest_client;
use agere_model_provider_info::ModelProviderInfo;
use agere_models_manager::manager::ModelsEndpointClient;
use agere_otel::TelemetryAuthMode;
use agere_protocol::error::Result as CoreResult;
use agere_protocol::openai_models::ModelInfo;
use agere_response_debug_context::extract_response_debug_context;
use agere_response_debug_context::telemetry_transport_error_message;
use async_trait::async_trait;
use http::HeaderMap;
use tokio::time::timeout;

use crate::auth::provider_has_bearer_auth_config;
use crate::auth::resolve_provider_auth;

const MODELS_REFRESH_TIMEOUT: Duration = Duration::from_secs(5);
const MODELS_ENDPOINT: &str = "/models";

/// Provider-owned OpenAI-compatible `/models` endpoint.
#[derive(Debug)]
pub(crate) struct OpenAiModelsEndpoint {
    provider_info: ModelProviderInfo,
    auth_manager: Option<Arc<AuthManager>>,
}

impl OpenAiModelsEndpoint {
    pub(crate) fn new(
        provider_info: ModelProviderInfo,
        auth_manager: Option<Arc<AuthManager>>,
    ) -> Self {
        Self {
            provider_info,
            auth_manager,
        }
    }

    async fn auth(&self) -> Option<AgereAuth> {
        match self.auth_manager.as_ref() {
            Some(auth_manager) => auth_manager.auth().await,
            None => None,
        }
    }

    fn auth_env(&self) -> AuthEnvTelemetry {
        let agere_api_key_env_enabled = self
            .auth_manager
            .as_ref()
            .is_some_and(|auth_manager| auth_manager.agere_api_key_env_enabled());
        collect_auth_env_telemetry(&self.provider_info, agere_api_key_env_enabled)
    }
}

#[async_trait]
impl ModelsEndpointClient for OpenAiModelsEndpoint {
    fn can_refresh_without_agere_backend(&self) -> bool {
        false
    }

    async fn uses_agere_backend(&self) -> bool {
        if provider_has_bearer_auth_config(&self.provider_info) {
            return false;
        }

        self.auth()
            .await
            .as_ref()
            .is_some_and(AgereAuth::uses_agere_backend)
    }

    async fn list_models(
        &self,
        client_version: &str,
    ) -> CoreResult<(Vec<ModelInfo>, Option<String>)> {
        let _timer =
            agere_otel::start_global_timer("agere.remote_models.fetch_update.duration_ms", &[]);
        let auth = self.auth().await;
        let auth_mode = auth.as_ref().map(AgereAuth::auth_mode);
        let api_provider = self.provider_info.to_api_provider(auth_mode)?;
        let api_auth = resolve_provider_auth(auth.as_ref(), &self.provider_info).await?;
        let auth_telemetry = auth_header_telemetry(api_auth.as_ref());
        let request_telemetry: Arc<dyn RequestTelemetry> = Arc::new(ModelsRequestTelemetry {
            auth_mode: auth_mode.map(|mode| TelemetryAuthMode::from(mode).to_string()),
            auth_header_attached: auth_telemetry.attached,
            auth_header_name: auth_telemetry.name,
            auth_env: self.auth_env(),
        });
        list_models_once(client_version, api_provider, api_auth, request_telemetry)
            .await
            .map_err(map_api_error)
    }
}

async fn list_models_once(
    client_version: &str,
    api_provider: agere_api::Provider,
    api_auth: agere_api::SharedAuthProvider,
    request_telemetry: Arc<dyn RequestTelemetry>,
) -> Result<(Vec<ModelInfo>, Option<String>), ApiError> {
    let transport = ReqwestTransport::new(build_reqwest_client());
    let client = ModelsClient::new(transport, api_provider, api_auth)
        .with_telemetry(Some(request_telemetry));

    timeout(
        MODELS_REFRESH_TIMEOUT,
        client.list_models(client_version, HeaderMap::new()),
    )
    .await
    .map_err(|_| ApiError::Transport(TransportError::Timeout))?
}

#[derive(Clone)]
struct ModelsRequestTelemetry {
    auth_mode: Option<String>,
    auth_header_attached: bool,
    auth_header_name: Option<&'static str>,
    auth_env: AuthEnvTelemetry,
}

impl RequestTelemetry for ModelsRequestTelemetry {
    fn on_request(
        &self,
        attempt: u64,
        status: Option<http::StatusCode>,
        error: Option<&TransportError>,
        duration: Duration,
    ) {
        let success = status.is_some_and(|code| code.is_success()) && error.is_none();
        let error_message = error.map(telemetry_transport_error_message);
        let response_debug = error
            .map(extract_response_debug_context)
            .unwrap_or_default();
        let status = status.map(|status| status.as_u16());
        tracing::event!(
            target: "agere_otel.log_only",
            tracing::Level::INFO,
            event.name = "agere.api_request",
            duration_ms = %duration.as_millis(),
            http.response.status_code = status,
            success = success,
            error.message = error_message.as_deref(),
            attempt = attempt,
            endpoint = MODELS_ENDPOINT,
            auth.header_attached = self.auth_header_attached,
            auth.header_name = self.auth_header_name,
            auth.env_openai_api_key_present = self.auth_env.openai_api_key_env_present,
            auth.env_agere_api_key_present = self.auth_env.agere_api_key_env_present,
            auth.env_agere_api_key_enabled = self.auth_env.agere_api_key_env_enabled,
            auth.env_provider_key_name = self.auth_env.provider_env_key_name.as_deref(),
            auth.env_provider_key_present = self.auth_env.provider_env_key_present,
            auth.request_id = response_debug.request_id.as_deref(),
            auth.cf_ray = response_debug.cf_ray.as_deref(),
            auth.error = response_debug.auth_error.as_deref(),
            auth.error_code = response_debug.auth_error_code.as_deref(),
            auth.mode = self.auth_mode.as_deref(),
        );
        tracing::event!(
            target: "agere_otel.trace_safe",
            tracing::Level::INFO,
            event.name = "agere.api_request",
            duration_ms = %duration.as_millis(),
            http.response.status_code = status,
            success = success,
            error.message = error_message.as_deref(),
            attempt = attempt,
            endpoint = MODELS_ENDPOINT,
            auth.header_attached = self.auth_header_attached,
            auth.header_name = self.auth_header_name,
            auth.env_openai_api_key_present = self.auth_env.openai_api_key_env_present,
            auth.env_agere_api_key_present = self.auth_env.agere_api_key_env_present,
            auth.env_agere_api_key_enabled = self.auth_env.agere_api_key_env_enabled,
            auth.env_provider_key_name = self.auth_env.provider_env_key_name.as_deref(),
            auth.env_provider_key_present = self.auth_env.provider_env_key_present,
            auth.request_id = response_debug.request_id.as_deref(),
            auth.cf_ray = response_debug.cf_ray.as_deref(),
            auth.error = response_debug.auth_error.as_deref(),
            auth.error_code = response_debug.auth_error_code.as_deref(),
            auth.mode = self.auth_mode.as_deref(),
        );
        emit_feedback_request_tags_with_auth_env(
            &FeedbackRequestTags {
                endpoint: MODELS_ENDPOINT,
                auth_header_attached: self.auth_header_attached,
                auth_header_name: self.auth_header_name,
                auth_mode: self.auth_mode.as_deref(),
                auth_retry_after_unauthorized: None,
                auth_recovery_mode: None,
                auth_recovery_phase: None,
                auth_connection_reused: None,
                auth_request_id: response_debug.request_id.as_deref(),
                auth_cf_ray: response_debug.cf_ray.as_deref(),
                auth_error: response_debug.auth_error.as_deref(),
                auth_error_code: response_debug.auth_error_code.as_deref(),
                auth_recovery_followup_success: None,
                auth_recovery_followup_status: None,
            },
            &self.auth_env,
        );
    }
}
