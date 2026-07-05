//! Shared builder for `ModelPreset` instances used by custom providers.
//!
//! Both `App::rebuild_config_for_cwd` and `App::handle_switch_provider`
//! construct identical `ModelPreset` vectors from a provider's model list
//! and `WireApi`. This module centralizes that logic.

use agere_config::config_toml::ModelConfig;
use agere_model_provider_info::WireApi;
use agere_models_manager::model_info::default_input_modalities_for_wire_api;
use agere_protocol::openai_models::ModelPreset;
use agere_protocol::openai_models::ReasoningEffort;
use agere_protocol::openai_models::ReasoningEffortPreset;

const ALL_EFFORTS: [ReasoningEffort; 7] = [
    ReasoningEffort::None,
    ReasoningEffort::Minimal,
    ReasoningEffort::Low,
    ReasoningEffort::Medium,
    ReasoningEffort::High,
    ReasoningEffort::XHigh,
    ReasoningEffort::Max,
];

/// Build reasoning effort presets filtered by the provider's wire API.
fn reasoning_effort_presets(wire_api: WireApi) -> Vec<ReasoningEffortPreset> {
    ALL_EFFORTS
        .iter()
        .filter(|effort| match wire_api {
            WireApi::Anthropic => effort.supported_in_anthropic(),
            WireApi::Chat => effort.supported_in_chat(),
            WireApi::Responses => effort.supported_in_responses(),
        })
        .map(|effort| ReasoningEffortPreset {
            effort: *effort,
            description: effort.description().to_string(),
        })
        .collect()
}

/// Get the default reasoning effort for a given wire API.
///
/// - WireApi::Anthropic | WireApi::Chat | WireApi::Responses → Medium
#[must_use]
pub fn default_reasoning_effort(wire_api: WireApi) -> ReasoningEffort {
    match wire_api {
        WireApi::Anthropic | WireApi::Chat | WireApi::Responses => ReasoningEffort::Medium,
    }
}

/// Build a list of `ModelPreset` entries from a provider's model configs.
///
/// - `wire_api` determines reasoning effort filters and default effort.
/// - `models` is the provider's model list (from `config.toml` or `provider.toml`).
/// - `default_model` is the model name that should have `is_default = true`.
#[must_use]
pub fn build_model_presets(
    wire_api: WireApi,
    models: &[ModelConfig],
    default_model: &str,
) -> Vec<ModelPreset> {
    let presets = reasoning_effort_presets(wire_api);
    let default_effort = default_reasoning_effort(wire_api);

    models
        .iter()
        .map(|m| {
            let name = m.name.clone();
            ModelPreset {
                id: name.clone(),
                model: name.clone(),
                display_name: name,
                description: String::new(),
                default_reasoning_effort: default_effort,
                supported_reasoning_efforts: presets.clone(),
                supports_personality: false,
                additional_speed_tiers: Vec::new(),
                is_default: m.name == default_model,
                upgrade: None,
                show_in_picker: true,
                availability_nux: None,
                supported_in_api: true,
                input_modalities: m
                    .input_modalities
                    .clone()
                    .unwrap_or_else(|| default_input_modalities_for_wire_api(wire_api)),
                context_window: m.context_window,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use agere_protocol::openai_models::InputModality;

    #[test]
    fn responses_wire_api_defaults_to_medium_reasoning() {
        assert_eq!(
            default_reasoning_effort(WireApi::Responses),
            ReasoningEffort::Medium
        );

        let presets = build_model_presets(
            WireApi::Responses,
            &[ModelConfig {
                name: "kk-model".to_string(),
                context_window: None,
                input_modalities: None,
            }],
            "kk-model",
        );

        assert_eq!(presets[0].default_reasoning_effort, ReasoningEffort::Medium);
    }

    #[test]
    fn model_config_input_modalities_are_preserved() {
        let presets = build_model_presets(
            WireApi::Responses,
            &[ModelConfig {
                name: "image-model".to_string(),
                context_window: None,
                input_modalities: Some(vec![InputModality::Text, InputModality::Image]),
            }],
            "image-model",
        );

        assert_eq!(
            presets[0].input_modalities,
            vec![InputModality::Text, InputModality::Image]
        );
    }

    #[test]
    fn omitted_model_config_input_modalities_use_wire_api_default() {
        let presets = build_model_presets(
            WireApi::Responses,
            &[ModelConfig {
                name: "legacy-model".to_string(),
                context_window: None,
                input_modalities: None,
            }],
            "legacy-model",
        );

        assert_eq!(presets[0].input_modalities, vec![InputModality::Text]);
    }
}
