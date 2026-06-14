use crate::types::OutputConfig;
use crate::types::ThinkingConfig;
use agere_protocol::openai_models::ReasoningEffort;

/// Convert internal ReasoningEffort to Anthropic adaptive thinking + output_config effort.
///
/// Opus 4.7: adaptive thinking only — budget_tokens returns 400.
/// Opus 4.6 / Sonnet 4.6: adaptive thinking recommended (budget_tokens deprecated).
///
/// Effort levels map to output_config.effort:
///   None/None -> disabled (no thinking, no effort override)
///   Minimal/Low -> low
///   Medium -> medium
///   High -> high (default)
///   XHigh -> xhigh (Opus 4.7 only — between high and max)
pub(crate) fn to_anthropic_thinking(effort: Option<ReasoningEffort>) -> Option<ThinkingConfig> {
    match effort {
        None | Some(ReasoningEffort::None) => None,
        Some(_) => Some(ThinkingConfig {
            thinking_type: "adaptive".into(),
            budget_tokens: None,
            display: None,
        }),
    }
}

/// Build output_config from ReasoningEffort.
/// Returns None when no effort override is needed (default is "high").
pub(crate) fn to_anthropic_output_config(effort: Option<ReasoningEffort>) -> Option<OutputConfig> {
    let level = match effort {
        None | Some(ReasoningEffort::None) => return None,
        Some(ReasoningEffort::Minimal) | Some(ReasoningEffort::Low) => "low",
        Some(ReasoningEffort::Medium) => "medium",
        Some(ReasoningEffort::High) => "high",
        Some(ReasoningEffort::XHigh) => "xhigh",
        Some(ReasoningEffort::Max) => "max",
    };
    Some(OutputConfig {
        effort: Some(level.into()),
        format: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reasoning_none_is_none() {
        assert_eq!(to_anthropic_thinking(None), None);
        assert_eq!(to_anthropic_output_config(None), None);
    }

    #[test]
    fn reasoning_minimal_maps_to_low() {
        let thinking = to_anthropic_thinking(Some(ReasoningEffort::Minimal));
        assert_eq!(
            thinking,
            Some(ThinkingConfig {
                thinking_type: "adaptive".into(),
                budget_tokens: None,
                display: None,
            })
        );
        let output = to_anthropic_output_config(Some(ReasoningEffort::Minimal));
        assert_eq!(output.unwrap().effort, Some("low".into()));
    }

    #[test]
    fn reasoning_low_maps_to_low() {
        let output = to_anthropic_output_config(Some(ReasoningEffort::Low));
        assert_eq!(output.unwrap().effort, Some("low".into()));
    }

    #[test]
    fn reasoning_medium_maps_to_medium() {
        let output = to_anthropic_output_config(Some(ReasoningEffort::Medium));
        assert_eq!(output.unwrap().effort, Some("medium".into()));
    }

    #[test]
    fn reasoning_high_maps_to_high() {
        let output = to_anthropic_output_config(Some(ReasoningEffort::High));
        assert_eq!(output.unwrap().effort, Some("high".into()));
    }

    #[test]
    fn reasoning_xhigh_maps_to_xhigh() {
        let output = to_anthropic_output_config(Some(ReasoningEffort::XHigh));
        assert_eq!(output.unwrap().effort, Some("xhigh".into()));
    }

    #[test]
    fn reasoning_max_maps_to_max() {
        let output = to_anthropic_output_config(Some(ReasoningEffort::Max));
        assert_eq!(output.unwrap().effort, Some("max".into()));
    }
}
