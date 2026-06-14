pub(crate) mod cache;
pub mod collaboration_mode_presets;
pub(crate) mod config;
pub mod manager;
pub mod model_info;
pub mod model_presets;
pub mod swappable;
pub mod test_support;

pub use agere_app_server_protocol::AuthMode;
pub use config::ModelsManagerConfig;
pub use swappable::SwappableModelsManager;

const LEGACY_GPT_IDENTITY_PREFIX: &str = "You are ";
const LEGACY_GPT_IDENTITY_SUFFIX: &str = "GPT-5.2 running in the Agere CLI, a terminal-based coding assistant. Agere CLI is an open source project led by OpenAI. You are expected to be precise, safe, and helpful.";
const AGERE_GPT_IDENTITY: &str = "You are Agere, powered by GPT-5.2. You are running in the Agere CLI, a terminal-based coding assistant. Agere CLI is an open source project by openagere. You are expected to be precise, safe, and helpful.";
const LEGACY_AGERE_BRANDING: &str = "Within this context, Agere refers to the open-source agentic coding interface (not the old Agere language model built by OpenAI).";
const AGERE_BRANDING: &str =
    "Within this context, Agere refers to the open-source agentic coding interface.";

/// Load the bundled model catalog shipped with `agere-models-manager`.
pub fn bundled_models_response()
-> std::result::Result<agere_protocol::openai_models::ModelsResponse, serde_json::Error> {
    let mut response: agere_protocol::openai_models::ModelsResponse =
        serde_json::from_str(include_str!("../models.json"))?;
    let legacy_identity = format!("{LEGACY_GPT_IDENTITY_PREFIX}{LEGACY_GPT_IDENTITY_SUFFIX}");
    for model in &mut response.models {
        model.base_instructions = model
            .base_instructions
            .replace(&legacy_identity, AGERE_GPT_IDENTITY);
        model.base_instructions = model
            .base_instructions
            .replace(LEGACY_AGERE_BRANDING, AGERE_BRANDING);
    }
    Ok(response)
}

/// Convert the client version string to a whole version string (e.g. "1.2.3-alpha.4" -> "1.2.3").
pub fn client_version_to_whole() -> String {
    format!(
        "{}.{}.{}",
        env!("CARGO_PKG_VERSION_MAJOR"),
        env!("CARGO_PKG_VERSION_MINOR"),
        env!("CARGO_PKG_VERSION_PATCH")
    )
}
