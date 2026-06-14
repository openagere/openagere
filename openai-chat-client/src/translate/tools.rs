use crate::ToolDefinition;
use crate::types::ChatFunctionChoiceName;
use crate::types::ChatTool;
use crate::types::ChatToolChoice;
use serde_json::Value;

pub(crate) fn to_chat_tools(defs: &[ToolDefinition]) -> Vec<ChatTool> {
    defs.iter()
        .map(|d| ChatTool {
            tool_type: "function".into(),
            function: crate::types::ChatFunctionDef {
                name: d.name.clone(),
                description: Some(d.description.clone()),
                parameters: d.parameters.clone(),
            },
        })
        .collect()
}

#[allow(dead_code)]
pub(crate) fn to_chat_tool_choice(tool_choice: &str) -> Option<ChatToolChoice> {
    match tool_choice {
        "auto" => Some(ChatToolChoice::Auto),
        "required" => Some(ChatToolChoice::Required),
        "none" => Some(ChatToolChoice::None),
        other => {
            if let Ok(val) = serde_json::from_str::<Value>(other)
                && let Some(name) = val
                    .get("function")
                    .and_then(|f| f.get("name"))
                    .and_then(|v| v.as_str())
            {
                return Some(ChatToolChoice::Function {
                    function: ChatFunctionChoiceName {
                        name: name.to_string(),
                    },
                });
            }
            Some(ChatToolChoice::Auto)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ToolDefinition as InternalToolDef;
    use serde_json::json;

    fn tool_def(name: &str, desc: &str, params: serde_json::Value) -> InternalToolDef {
        InternalToolDef {
            name: name.into(),
            description: desc.into(),
            parameters: params,
        }
    }

    #[test]
    fn single_tool_conversion() {
        let defs = vec![tool_def(
            "get_weather",
            "Get weather for a city",
            json!({"type": "object", "properties": {"city": {"type": "string"}}}),
        )];
        let result = to_chat_tools(&defs);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].tool_type, "function");
        assert_eq!(result[0].function.name, "get_weather");
        assert_eq!(
            result[0].function.description.as_deref(),
            Some("Get weather for a city")
        );
    }

    #[test]
    fn empty_description_is_empty_string() {
        let defs = vec![tool_def("test", "", json!({}))];
        let result = to_chat_tools(&defs);
        assert_eq!(result[0].function.description, Some("".into()));
    }

    #[test]
    fn empty_tools_returns_empty_vec() {
        let result = to_chat_tools(&[]);
        assert!(result.is_empty());
    }

    #[test]
    fn tool_choice_auto() {
        assert_eq!(to_chat_tool_choice("auto"), Some(ChatToolChoice::Auto));
    }

    #[test]
    fn tool_choice_required() {
        assert_eq!(
            to_chat_tool_choice("required"),
            Some(ChatToolChoice::Required)
        );
    }

    #[test]
    fn tool_choice_none() {
        assert_eq!(to_chat_tool_choice("none"), Some(ChatToolChoice::None));
    }

    #[test]
    fn tool_choice_specific_name() {
        let choice = r#"{"type":"function","function":{"name":"get_weather"}}"#;
        assert_eq!(
            to_chat_tool_choice(choice),
            Some(ChatToolChoice::Function {
                function: ChatFunctionChoiceName {
                    name: "get_weather".into(),
                },
            })
        );
    }
}
