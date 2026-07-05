use super::*;
use crate::ModelsManagerConfig;
use agere_model_provider_info::WireApi;
use agere_protocol::openai_models::InputModality;
use pretty_assertions::assert_eq;

#[test]
fn reasoning_summaries_override_true_enables_support() {
    let model = model_info_from_slug("unknown-model");
    let config = ModelsManagerConfig {
        model_supports_reasoning_summaries: Some(true),
        ..Default::default()
    };

    let updated = with_config_overrides(model.clone(), &config);
    let mut expected = model;
    expected.supports_reasoning_summaries = true;

    assert_eq!(updated, expected);
}

#[test]
fn reasoning_summaries_override_false_does_not_disable_support() {
    let mut model = model_info_from_slug("unknown-model");
    model.supports_reasoning_summaries = true;
    let config = ModelsManagerConfig {
        model_supports_reasoning_summaries: Some(false),
        ..Default::default()
    };

    let updated = with_config_overrides(model.clone(), &config);

    assert_eq!(updated, model);
}

#[test]
fn wire_api_fallback_is_text_only_for_responses() {
    let model = model_info_from_slug_for_wire_api("unknown-responses", WireApi::Responses);
    assert_eq!(model.input_modalities, Some(vec![InputModality::Text]));
    assert!(model.used_fallback_model_metadata);
}

#[test]
fn wire_api_fallback_is_text_only_for_chat() {
    let model = model_info_from_slug_for_wire_api("unknown-chat", WireApi::Chat);
    assert_eq!(model.input_modalities, Some(vec![InputModality::Text]));
    assert!(model.used_fallback_model_metadata);
}

#[test]
fn wire_api_fallback_is_text_only_for_anthropic() {
    let model = model_info_from_slug_for_wire_api("unknown-anthropic", WireApi::Anthropic);
    assert_eq!(model.input_modalities, Some(vec![InputModality::Text]));
    assert!(model.used_fallback_model_metadata);
}

#[test]
fn reasoning_summaries_override_false_is_noop_when_model_is_false() {
    let model = model_info_from_slug("unknown-model");
    let config = ModelsManagerConfig {
        model_supports_reasoning_summaries: Some(false),
        ..Default::default()
    };

    let updated = with_config_overrides(model.clone(), &config);

    assert_eq!(updated, model);
}

#[test]
fn model_context_window_override_clamps_to_max_context_window() {
    let mut model = model_info_from_slug("unknown-model");
    model.context_window = Some(273_000);
    model.max_context_window = Some(400_000);
    let config = ModelsManagerConfig {
        model_context_window: Some(500_000),
        ..Default::default()
    };

    let updated = with_config_overrides(model.clone(), &config);
    let mut expected = model;
    expected.context_window = Some(400_000);

    assert_eq!(updated, expected);
}

#[test]
fn model_context_window_uses_model_value_without_override() {
    let mut model = model_info_from_slug("unknown-model");
    model.context_window = Some(273_000);
    model.max_context_window = Some(400_000);
    let config = ModelsManagerConfig::default();

    let updated = with_config_overrides(model.clone(), &config);

    assert_eq!(updated, model);
}
