use crate::ToolDefinition;
use crate::types::AnthropicTool;
use crate::types::ToolChoice;
use serde_json::Value;

/// Convert internal ToolDefinition slice to Anthropic tool definitions.
/// Returns `None` for empty input (Anthropic expects no `tools` field).
/// Enables `strict: true` for structured output validation on Opus 4.7 / Sonnet 4.6 / Haiku 4.5.
pub(crate) fn to_anthropic_tools(defs: &[ToolDefinition]) -> Option<Vec<AnthropicTool>> {
    if defs.is_empty() {
        return None;
    }
    Some(
        defs.iter()
            .map(|d| AnthropicTool {
                name: d.name.clone(),
                description: if d.description.is_empty() {
                    None
                } else {
                    Some(d.description.clone())
                },
                input_schema: d.parameters.clone(),
                strict: Some(true),
            })
            .collect(),
    )
}

/// Convert tool_choice string or JSON to Anthropic ToolChoice.
/// Values: "auto", "required", "none", or {"type":"function","function":{"name":"x"}}
#[allow(dead_code)]
pub(crate) fn to_anthropic_tool_choice(tool_choice: &str) -> Option<ToolChoice> {
    match tool_choice {
        "auto" => Some(ToolChoice::Auto),
        "required" => Some(ToolChoice::Any),
        "none" => None,
        other => {
            if let Ok(val) = serde_json::from_str::<Value>(other)
                && let Some(name) = val
                    .get("function")
                    .and_then(|f| f.get("name"))
                    .and_then(|v| v.as_str())
            {
                return Some(ToolChoice::Tool {
                    name: name.to_string(),
                });
            }
            Some(ToolChoice::Auto)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn td(name: &str, desc: &str, params: serde_json::Value) -> ToolDefinition {
        ToolDefinition {
            name: name.into(),
            description: desc.into(),
            parameters: params,
        }
    }

    #[test]
    fn single_tool_conversion() {
        let defs = vec![td(
            "get_weather",
            "Get weather for a city",
            json!({"type": "object", "properties": {"city": {"type": "string"}}}),
        )];
        let result = to_anthropic_tools(&defs);
        assert!(result.is_some());
        let tools = result.unwrap();
        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0].name, "get_weather");
        assert_eq!(
            tools[0].description.as_deref(),
            Some("Get weather for a city")
        );
        assert_eq!(
            tools[0].input_schema,
            json!({"type": "object", "properties": {"city": {"type": "string"}}})
        );
        assert_eq!(tools[0].strict, Some(true));
    }

    #[test]
    fn empty_description_is_none() {
        let defs = vec![td("test", "", json!({}))];
        let result = to_anthropic_tools(&defs);
        assert_eq!(result.unwrap()[0].description, None);
    }

    #[test]
    fn empty_tools_returns_none() {
        let result = to_anthropic_tools(&[]);
        assert!(result.is_none());
    }

    #[test]
    fn tool_choice_auto() {
        assert_eq!(to_anthropic_tool_choice("auto"), Some(ToolChoice::Auto));
    }

    #[test]
    fn tool_choice_required() {
        assert_eq!(to_anthropic_tool_choice("required"), Some(ToolChoice::Any));
    }

    #[test]
    fn tool_choice_none() {
        assert_eq!(to_anthropic_tool_choice("none"), None);
    }

    #[test]
    fn tool_choice_specific_name() {
        let choice = r#"{"type":"function","function":{"name":"get_weather"}}"#;
        assert_eq!(
            to_anthropic_tool_choice(choice),
            Some(ToolChoice::Tool {
                name: "get_weather".into()
            })
        );
    }
}
