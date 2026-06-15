//! Build ModelsResponse from user-defined ModelConfig entries.

use agere_model_provider_info::WireApi;
use agere_protocol::config_types::ReasoningSummary;
use agere_protocol::openai_models::ApplyPatchToolType;
use agere_protocol::openai_models::ConfigShellToolType;
use agere_protocol::openai_models::InputModality;
use agere_protocol::openai_models::ModelInfo;
use agere_protocol::openai_models::ModelVisibility;
use agere_protocol::openai_models::ModelsResponse;
use agere_protocol::openai_models::ReasoningEffort;
use agere_protocol::openai_models::ReasoningEffortPreset;
use agere_protocol::openai_models::TruncationPolicyConfig;
use agere_protocol::openai_models::WebSearchToolType;

use agere_config::config_toml::ModelConfig;
use agere_models_manager::model_info::BASE_INSTRUCTIONS;

const DEFAULT_CONTEXT_WINDOW: i64 = 200_000;

/// Build a ModelsResponse from user-defined model configs.
pub fn build_models_response(models: &[ModelConfig], wire_api: WireApi) -> ModelsResponse {
    ModelsResponse {
        models: models
            .iter()
            .enumerate()
            .map(|(i, cfg)| model_from_config(cfg, wire_api, i as i32))
            .collect(),
    }
}

fn model_from_config(cfg: &ModelConfig, wire_api: WireApi, priority: i32) -> ModelInfo {
    let context_window = cfg.context_window.unwrap_or(DEFAULT_CONTEXT_WINDOW);
    ModelInfo {
        slug: cfg.name.clone(),
        display_name: cfg.name.clone(),
        description: None,
        default_reasoning_level: Some(ReasoningEffort::Medium),
        supported_reasoning_levels: reasoning_levels_for_wire_api(wire_api),
        shell_type: ConfigShellToolType::ShellCommand,
        visibility: ModelVisibility::List,
        supported_in_api: true,
        priority,
        additional_speed_tiers: Vec::new(),
        availability_nux: None,
        upgrade: None,
        base_instructions: BASE_INSTRUCTIONS.to_string(),
        model_messages: None,
        supports_reasoning_summaries: matches!(wire_api, WireApi::Anthropic),
        default_reasoning_summary: ReasoningSummary::None,
        support_verbosity: false,
        default_verbosity: None,
        apply_patch_tool_type: Some(ApplyPatchToolType::Function),
        web_search_tool_type: WebSearchToolType::Text,
        truncation_policy: TruncationPolicyConfig::tokens(10_000),
        supports_parallel_tool_calls: true,
        supports_image_detail_original: false,
        context_window: Some(context_window),
        max_context_window: Some(context_window),
        auto_compact_token_limit: None,
        effective_context_window_percent: 95,
        experimental_supported_tools: Vec::new(),
        input_modalities: vec![InputModality::Text],
        used_fallback_model_metadata: false,
        supports_search_tool: false,
    }
}

fn reasoning_levels_for_wire_api(wire_api: WireApi) -> Vec<ReasoningEffortPreset> {
    let descriptions: &[(ReasoningEffort, &str)] = &[
        (ReasoningEffort::None, "No reasoning"),
        (ReasoningEffort::Minimal, "Minimal"),
        (ReasoningEffort::Low, "Low"),
        (ReasoningEffort::Medium, "Medium"),
        (ReasoningEffort::High, "High"),
        (ReasoningEffort::XHigh, "Extra high"),
        (ReasoningEffort::Max, "Max"),
    ];

    let supported = |e: ReasoningEffort| match wire_api {
        WireApi::Anthropic => e.supported_in_anthropic(),
        WireApi::Chat => e.supported_in_chat(),
        WireApi::Responses => e.supported_in_responses(),
    };

    descriptions
        .iter()
        .filter(|(e, _)| supported(*e))
        .map(|(e, desc)| ReasoningEffortPreset {
            effort: *e,
            description: desc.to_string(),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use agere_model_provider_info::WireApi;

    use super::*;

    #[test]
    fn builds_models_response_from_config() {
        let models = vec![
            ModelConfig {
                name: "deepseek-v4-pro".to_string(),
                context_window: Some(200_000),
            },
            ModelConfig {
                name: "claude-sonnet-4-6".to_string(),
                context_window: None,
            },
        ];

        let response = build_models_response(&models, WireApi::Anthropic);

        assert_eq!(response.models.len(), 2);
        assert_eq!(response.models[0].slug, "deepseek-v4-pro");
        assert_eq!(response.models[0].context_window, Some(200_000));
        assert_eq!(response.models[1].slug, "claude-sonnet-4-6");
        assert_eq!(
            response.models[1].context_window,
            Some(DEFAULT_CONTEXT_WINDOW)
        );
        assert!(!response.models[0].used_fallback_model_metadata);
    }

    #[test]
    fn anthropic_api_has_limited_reasoning_levels() {
        let models = vec![ModelConfig {
            name: "test".to_string(),
            context_window: None,
        }];
        let response = build_models_response(&models, WireApi::Anthropic);
        let efforts: Vec<_> = response.models[0]
            .supported_reasoning_levels
            .iter()
            .map(|p| p.effort)
            .collect();
        assert_eq!(
            efforts,
            vec![
                ReasoningEffort::Low,
                ReasoningEffort::Medium,
                ReasoningEffort::High,
                ReasoningEffort::XHigh,
                ReasoningEffort::Max,
            ]
        );
        assert!(!efforts.contains(&ReasoningEffort::None));
        assert!(!efforts.contains(&ReasoningEffort::Minimal));
    }

    #[test]
    fn responses_api_has_full_reasoning_levels() {
        let models = vec![ModelConfig {
            name: "test".to_string(),
            context_window: None,
        }];
        let response = build_models_response(&models, WireApi::Responses);
        let efforts: Vec<_> = response.models[0]
            .supported_reasoning_levels
            .iter()
            .map(|p| p.effort)
            .collect();
        assert_eq!(
            efforts,
            vec![
                ReasoningEffort::None,
                ReasoningEffort::Low,
                ReasoningEffort::Medium,
                ReasoningEffort::High,
                ReasoningEffort::XHigh,
            ]
        );
        assert!(!efforts.contains(&ReasoningEffort::Minimal));
        assert!(!efforts.contains(&ReasoningEffort::Max));
    }

    #[test]
    fn chat_api_has_chat_reasoning_levels() {
        let models = vec![ModelConfig {
            name: "test".to_string(),
            context_window: None,
        }];
        let response = build_models_response(&models, WireApi::Chat);
        let efforts: Vec<_> = response.models[0]
            .supported_reasoning_levels
            .iter()
            .map(|p| p.effort)
            .collect();
        assert_eq!(
            efforts,
            vec![
                ReasoningEffort::Minimal,
                ReasoningEffort::Low,
                ReasoningEffort::Medium,
                ReasoningEffort::High,
            ]
        );
        assert!(!efforts.contains(&ReasoningEffort::None));
        assert!(!efforts.contains(&ReasoningEffort::XHigh));
        assert!(!efforts.contains(&ReasoningEffort::Max));
    }
}
