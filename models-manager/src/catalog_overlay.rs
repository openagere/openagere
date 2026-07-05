use agere_protocol::openai_models::ApplyPatchToolType;
use agere_protocol::openai_models::InputModality;
use agere_protocol::openai_models::ModelInfo;
use agere_protocol::openai_models::ModelsResponse;
use agere_protocol::openai_models::ReasoningEffort;
use agere_protocol::openai_models::ReasoningEffortPreset;
use agere_protocol::openai_models::WebSearchToolType;
use serde::Deserialize;
use serde::Serialize;

/// Wire-api catalog metadata for one canonical model slug.
///
/// This intentionally excludes provider-owned presentation and sizing fields
/// such as display name and context window.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CatalogModel {
    pub(crate) slug: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) input_modalities: Option<Vec<InputModality>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) default_reasoning_level: Option<ReasoningEffort>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) supported_reasoning_levels: Vec<ReasoningEffortPreset>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub(crate) supports_reasoning_summaries: bool,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub(crate) supports_parallel_tool_calls: bool,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub(crate) supports_image_detail_original: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) apply_patch_tool_type: Option<ApplyPatchToolType>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) experimental_supported_tools: Vec<String>,
    #[serde(default)]
    pub(crate) web_search_tool_type: WebSearchToolType,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub(crate) supports_search_tool: bool,
}

impl From<ModelInfo> for CatalogModel {
    fn from(model: ModelInfo) -> Self {
        Self {
            slug: model.slug,
            input_modalities: model.input_modalities,
            default_reasoning_level: model.default_reasoning_level,
            supported_reasoning_levels: model.supported_reasoning_levels,
            supports_reasoning_summaries: model.supports_reasoning_summaries,
            supports_parallel_tool_calls: model.supports_parallel_tool_calls,
            supports_image_detail_original: model.supports_image_detail_original,
            apply_patch_tool_type: model.apply_patch_tool_type,
            experimental_supported_tools: model.experimental_supported_tools,
            web_search_tool_type: model.web_search_tool_type,
            supports_search_tool: model.supports_search_tool,
        }
    }
}

/// A set of catalog models used only to overlay capability metadata.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CatalogOverlay {
    pub(crate) models: Vec<CatalogModel>,
}

impl CatalogOverlay {
    pub(crate) fn from_models_response(response: &ModelsResponse) -> Self {
        Self {
            models: response
                .models
                .iter()
                .cloned()
                .map(CatalogModel::from)
                .collect(),
        }
    }
}

pub(crate) fn apply_catalog_overlay(mut base: ModelInfo, overlays: &[CatalogOverlay]) -> ModelInfo {
    let Some(overlay) = find_overlay_model(&base.slug, overlays) else {
        return base;
    };

    for catalog in overlays {
        let Some(modality_overlay) =
            find_overlay_model(base.slug.as_str(), std::slice::from_ref(catalog))
        else {
            continue;
        };
        if should_overlay_input_modalities(&base, modality_overlay) {
            base.input_modalities = modality_overlay.input_modalities.clone();
            break;
        }
    }
    if base.supported_reasoning_levels.is_empty() {
        base.supported_reasoning_levels = overlay.supported_reasoning_levels.clone();
    }
    if base.default_reasoning_level.is_none() || base.used_fallback_model_metadata {
        base.default_reasoning_level = overlay.default_reasoning_level;
    }
    if !base.supports_reasoning_summaries {
        base.supports_reasoning_summaries = overlay.supports_reasoning_summaries;
    }
    if base.used_fallback_model_metadata && !base.supports_parallel_tool_calls {
        base.supports_parallel_tool_calls = overlay.supports_parallel_tool_calls;
    }
    if !base.supports_image_detail_original {
        base.supports_image_detail_original = overlay.supports_image_detail_original;
    }
    if base.apply_patch_tool_type.is_none() {
        base.apply_patch_tool_type = overlay.apply_patch_tool_type.clone();
    }
    if base.used_fallback_model_metadata && !base.supports_search_tool {
        base.supports_search_tool = overlay.supports_search_tool;
    }
    if base.experimental_supported_tools.is_empty() {
        base.experimental_supported_tools = overlay.experimental_supported_tools.clone();
    }

    if base.web_search_tool_type == WebSearchToolType::Text {
        base.web_search_tool_type = overlay.web_search_tool_type;
    }
    base
}

fn should_overlay_input_modalities(base: &ModelInfo, overlay: &CatalogModel) -> bool {
    overlay.input_modalities.is_some()
        && (base.input_modalities.is_none() || base.used_fallback_model_metadata)
}

pub(crate) fn find_overlay_model<'a>(
    slug: &str,
    overlays: &'a [CatalogOverlay],
) -> Option<&'a CatalogModel> {
    for overlay in overlays {
        if let Some(model) = find_model_by_longest_prefix(slug, &overlay.models) {
            return Some(model);
        }
        if let Some(base_slug) = single_segment_namespaced_suffix(slug)
            && let Some(model) = find_model_by_longest_prefix(base_slug, &overlay.models)
        {
            return Some(model);
        }
    }
    None
}

fn find_model_by_longest_prefix<'a>(
    slug: &str,
    candidates: &'a [CatalogModel],
) -> Option<&'a CatalogModel> {
    let mut best: Option<&CatalogModel> = None;
    for candidate in candidates {
        if !slug.starts_with(&candidate.slug) {
            continue;
        }
        let is_better_match = best
            .as_ref()
            .is_none_or(|current| candidate.slug.len() > current.slug.len());
        if is_better_match {
            best = Some(candidate);
        }
    }
    best
}

fn single_segment_namespaced_suffix(slug: &str) -> Option<&str> {
    let (namespace, suffix) = slug.split_once('/')?;
    if suffix.contains('/') {
        return None;
    }
    if !namespace
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_')
    {
        return None;
    }
    Some(suffix)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model_info::model_info_from_slug;
    use crate::model_info::model_info_from_slug_for_wire_api;
    use agere_model_provider_info::WireApi;
    use agere_protocol::openai_models::InputModality;
    use agere_protocol::openai_models::ReasoningEffort;
    use agere_protocol::openai_models::ReasoningEffortPreset;
    use pretty_assertions::assert_eq;

    fn catalog_overlay(models: Vec<ModelInfo>) -> CatalogOverlay {
        CatalogOverlay {
            models: models.into_iter().map(CatalogModel::from).collect(),
        }
    }

    #[test]
    fn overlay_fills_input_modalities_without_overwriting_context_window() {
        let mut base = model_info_from_slug("qwen-plus");
        base.context_window = Some(1_000_000);
        base.input_modalities = None;

        let mut overlay = model_info_from_slug("qwen-plus");
        overlay.context_window = Some(200_000);
        overlay.input_modalities = Some(vec![InputModality::Text]);

        let actual = apply_catalog_overlay(base, &[catalog_overlay(vec![overlay])]);

        assert_eq!(actual.context_window, Some(1_000_000));
        assert_eq!(actual.input_modalities, Some(vec![InputModality::Text]));
    }

    #[test]
    fn overlay_uses_first_matching_catalog_by_priority() {
        let mut base = model_info_from_slug("same-model");
        base.input_modalities = None;

        let mut local = model_info_from_slug("same-model");
        local.input_modalities = Some(vec![InputModality::Text, InputModality::Image]);

        let mut remote = model_info_from_slug("same-model");
        remote.input_modalities = Some(vec![InputModality::Text]);

        let actual = apply_catalog_overlay(
            base,
            &[catalog_overlay(vec![local]), catalog_overlay(vec![remote])],
        );

        assert_eq!(
            actual.input_modalities,
            Some(vec![InputModality::Text, InputModality::Image])
        );
    }

    #[test]
    fn overlay_replaces_text_only_fallback_modalities_when_catalog_is_explicit() {
        let base = model_info_from_slug_for_wire_api("image-capable-fallback", WireApi::Responses);
        assert_eq!(base.input_modalities, Some(vec![InputModality::Text]));

        let mut overlay = model_info_from_slug("image-capable-fallback");
        overlay.input_modalities = Some(vec![InputModality::Text, InputModality::Image]);

        let actual = apply_catalog_overlay(base, &[catalog_overlay(vec![overlay])]);

        assert_eq!(
            actual.input_modalities,
            Some(vec![InputModality::Text, InputModality::Image])
        );
    }

    #[test]
    fn overlay_replaced_modalities_remain_explicit_after_json_roundtrip() {
        let mut base = model_info_from_slug("text-only-after-overlay");
        base.input_modalities = None;

        let mut overlay = model_info_from_slug("text-only-after-overlay");
        overlay.input_modalities = Some(vec![InputModality::Text]);

        let actual = apply_catalog_overlay(base, &[catalog_overlay(vec![overlay])]);
        assert_eq!(actual.input_modalities, Some(vec![InputModality::Text]));

        let roundtripped: ModelInfo =
            serde_json::from_value(serde_json::to_value(actual).expect("serialize model"))
                .expect("deserialize model");
        assert_eq!(
            roundtripped.input_modalities,
            Some(vec![InputModality::Text])
        );
    }

    #[test]
    fn defaulted_catalog_modalities_do_not_shadow_later_explicit_overlay() {
        let mut base = model_info_from_slug("same-model");
        base.input_modalities = None;

        let mut local = model_info_from_slug("same-model");
        local.input_modalities = None;

        let mut remote = model_info_from_slug("same-model");
        remote.input_modalities = Some(vec![InputModality::Text]);

        let actual = apply_catalog_overlay(
            base,
            &[catalog_overlay(vec![local]), catalog_overlay(vec![remote])],
        );

        assert_eq!(actual.input_modalities, Some(vec![InputModality::Text]));
    }

    #[test]
    fn overlay_preserves_explicit_provider_modalities() {
        let mut base = model_info_from_slug("explicit-image-model");
        base.input_modalities = Some(vec![InputModality::Text, InputModality::Image]);
        base.used_fallback_model_metadata = false;

        let mut overlay = model_info_from_slug("explicit-image-model");
        overlay.input_modalities = Some(vec![InputModality::Text]);

        let actual = apply_catalog_overlay(base, &[catalog_overlay(vec![overlay])]);

        assert_eq!(
            actual.input_modalities,
            Some(vec![InputModality::Text, InputModality::Image])
        );
    }

    #[test]
    fn overlay_matches_single_segment_namespaced_alias_by_base_slug() {
        let base = model_info_from_slug_for_wire_api("openrouter/gpt-image", WireApi::Responses);
        assert_eq!(base.input_modalities, Some(vec![InputModality::Text]));

        let mut overlay = model_info_from_slug("gpt-image");
        overlay.input_modalities = Some(vec![InputModality::Text, InputModality::Image]);

        let actual = apply_catalog_overlay(base, &[catalog_overlay(vec![overlay])]);

        assert_eq!(
            actual.input_modalities,
            Some(vec![InputModality::Text, InputModality::Image])
        );
    }

    #[test]
    fn overlay_matches_longest_prefix_alias() {
        let base = model_info_from_slug_for_wire_api("gpt-overlay-experiment", WireApi::Responses);
        assert_eq!(base.input_modalities, Some(vec![InputModality::Text]));

        let mut overlay = model_info_from_slug("gpt-overlay");
        overlay.input_modalities = Some(vec![InputModality::Text, InputModality::Image]);

        let actual = apply_catalog_overlay(base, &[catalog_overlay(vec![overlay])]);

        assert_eq!(
            actual.input_modalities,
            Some(vec![InputModality::Text, InputModality::Image])
        );
    }

    #[test]
    fn overlay_fills_reasoning_when_base_has_none() {
        let mut base = model_info_from_slug("reasoning-model");
        base.default_reasoning_level = None;
        base.supported_reasoning_levels = Vec::new();

        let mut overlay = model_info_from_slug("reasoning-model");
        overlay.default_reasoning_level = Some(ReasoningEffort::Medium);
        overlay.supported_reasoning_levels = vec![ReasoningEffortPreset {
            effort: ReasoningEffort::Medium,
            description: "medium".to_string(),
        }];

        let actual = apply_catalog_overlay(base, &[catalog_overlay(vec![overlay])]);

        assert_eq!(
            actual.default_reasoning_level,
            Some(ReasoningEffort::Medium)
        );
        assert_eq!(actual.supported_reasoning_levels.len(), 1);
    }

    #[test]
    fn overlay_replaces_fallback_reasoning_default() {
        let mut base = model_info_from_slug_for_wire_api("fallback-reasoning", WireApi::Responses);
        base.default_reasoning_level = Some(ReasoningEffort::Medium);
        base.supported_reasoning_levels = Vec::new();
        assert!(base.used_fallback_model_metadata);

        let mut overlay = model_info_from_slug("fallback-reasoning");
        overlay.default_reasoning_level = Some(ReasoningEffort::High);
        overlay.supported_reasoning_levels = vec![ReasoningEffortPreset {
            effort: ReasoningEffort::High,
            description: "high".to_string(),
        }];

        let actual = apply_catalog_overlay(base, &[catalog_overlay(vec![overlay])]);

        assert_eq!(actual.default_reasoning_level, Some(ReasoningEffort::High));
        assert_eq!(
            actual.supported_reasoning_levels,
            vec![ReasoningEffortPreset {
                effort: ReasoningEffort::High,
                description: "high".to_string(),
            }]
        );
    }

    #[test]
    fn overlay_does_not_enable_explicitly_disabled_provider_tool_capabilities() {
        let mut base = model_info_from_slug("gateway-model");
        base.supports_parallel_tool_calls = false;
        base.supports_search_tool = false;
        base.used_fallback_model_metadata = false;

        let mut overlay = model_info_from_slug("gateway-model");
        overlay.supports_parallel_tool_calls = true;
        overlay.supports_search_tool = true;

        let actual = apply_catalog_overlay(base, &[catalog_overlay(vec![overlay])]);

        assert!(!actual.supports_parallel_tool_calls);
        assert!(!actual.supports_search_tool);
    }

    #[test]
    fn overlay_fills_tool_capabilities_for_fallback_metadata() {
        let base = model_info_from_slug_for_wire_api("fallback-tools", WireApi::Responses);
        assert!(base.used_fallback_model_metadata);
        assert!(!base.supports_parallel_tool_calls);
        assert!(!base.supports_search_tool);

        let mut overlay = model_info_from_slug("fallback-tools");
        overlay.supports_parallel_tool_calls = true;
        overlay.supports_search_tool = true;

        let actual = apply_catalog_overlay(base, &[catalog_overlay(vec![overlay])]);

        assert!(actual.supports_parallel_tool_calls);
        assert!(actual.supports_search_tool);
    }

    #[test]
    fn overlay_does_not_downgrade_web_search_tool_type() {
        let mut base = model_info_from_slug("web-search-model");
        base.web_search_tool_type = agere_protocol::openai_models::WebSearchToolType::TextAndImage;

        let overlay = model_info_from_slug("web-search-model");
        assert_eq!(
            overlay.web_search_tool_type,
            agere_protocol::openai_models::WebSearchToolType::Text
        );

        let actual = apply_catalog_overlay(base, &[catalog_overlay(vec![overlay])]);

        assert_eq!(
            actual.web_search_tool_type,
            agere_protocol::openai_models::WebSearchToolType::TextAndImage
        );
    }
}
