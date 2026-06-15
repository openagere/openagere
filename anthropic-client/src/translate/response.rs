use crate::error::map_anthropic_error;
use crate::types::*;
use agere_api::ApiError;
use agere_api::ResponseEvent;
use agere_protocol::models::ContentItem;
use agere_protocol::models::ReasoningItemContent;
use agere_protocol::models::ResponseItem;
use agere_protocol::protocol::TokenUsage;
use std::collections::HashMap;
use tracing::debug;

/// SSE stream processing state.
#[derive(Debug)]
pub(crate) struct SseState {
    pub response_id: Option<String>,
    pub server_model: Option<String>,
    pub blocks: HashMap<u32, BlockState>,
}

#[derive(Debug)]
pub(crate) enum BlockState {
    Text {
        buffer: String,
    },
    ToolUse {
        id: String,
        name: String,
        partial_json: String,
    },
    Thinking {
        buffer: String,
        signature: Option<String>,
    },
    RedactedThinking {
        data: String,
        signature: Option<String>,
    },
}

impl SseState {
    pub fn new() -> Self {
        Self {
            response_id: None,
            server_model: None,
            blocks: HashMap::new(),
        }
    }

    pub fn ensure_block(&mut self, index: u32, block: &ContentBlockStartInfo) {
        if self.blocks.contains_key(&index) {
            return;
        }
        let state = match block {
            ContentBlockStartInfo::Text { .. } => BlockState::Text {
                buffer: String::new(),
            },
            ContentBlockStartInfo::ToolUse { id, name } => BlockState::ToolUse {
                id: id.clone(),
                name: name.clone(),
                partial_json: String::new(),
            },
            ContentBlockStartInfo::Thinking { thinking } => BlockState::Thinking {
                buffer: thinking.clone(),
                signature: None,
            },
            ContentBlockStartInfo::RedactedThinking { data } => BlockState::RedactedThinking {
                data: data.clone(),
                signature: None,
            },
        };
        self.blocks.insert(index, state);
    }
}

/// Handle a single parsed SSE event.
/// Returns an empty vec for no-op events (ping, message_start, message_stop).
pub(crate) fn handle_sse_event(
    event: &SseEvent,
    state: &mut SseState,
) -> Vec<Result<ResponseEvent, ApiError>> {
    match event {
        SseEvent::MessageStart { message } => {
            state.response_id = Some(message.id.clone());
            state.server_model = Some(message.model.clone());
            vec![]
        }
        SseEvent::ContentBlockStart {
            index,
            content_block,
        } => {
            state.ensure_block(*index, content_block);
            match content_block {
                ContentBlockStartInfo::ToolUse { id, name } => {
                    let item = ResponseItem::FunctionCall {
                        id: Some(id.clone()),
                        call_id: id.clone(),
                        name: name.clone(),
                        arguments: String::new(),
                        namespace: None,
                    };
                    vec![Ok(ResponseEvent::OutputItemAdded(item))]
                }
                ContentBlockStartInfo::Thinking { thinking } => {
                    debug!(
                        "SSE content_block_start thinking idx={} initial_text_len={} initial_text_preview={:?}",
                        index,
                        thinking.len(),
                        &thinking[..thinking.len().min(80)]
                    );
                    let item = ResponseItem::Reasoning {
                        id: format!("rs_{index}"),
                        summary: vec![],
                        content: Some(vec![]),
                        encrypted_content: None,
                        signature: None,
                    };
                    vec![Ok(ResponseEvent::OutputItemAdded(item))]
                }
                ContentBlockStartInfo::RedactedThinking { data } => {
                    debug!(
                        "SSE content_block_start redacted_thinking idx={} data_len={}",
                        index,
                        data.len()
                    );
                    let item = ResponseItem::Reasoning {
                        id: format!("rs_{index}"),
                        summary: vec![],
                        content: None,
                        encrypted_content: Some(data.clone()),
                        signature: None,
                    };
                    vec![Ok(ResponseEvent::OutputItemAdded(item))]
                }
                ContentBlockStartInfo::Text { text } => {
                    // Always create a message item, even if text is empty.
                    // This ensures active_item is set before TextDelta arrives.
                    let content = vec![ContentItem::OutputText { text: text.clone() }];
                    let item = ResponseItem::Message {
                        id: None,
                        role: "assistant".into(),
                        content,
                        phase: None,
                    };
                    vec![Ok(ResponseEvent::OutputItemAdded(item))]
                }
            }
        }
        SseEvent::ContentBlockDelta { index, delta } => {
            match delta {
                Delta::TextDelta { text } => {
                    if let Some(BlockState::Text { buffer }) = state.blocks.get_mut(index) {
                        buffer.push_str(text);
                    }
                    return vec![Ok(ResponseEvent::OutputTextDelta(text.clone()))];
                }
                Delta::InputJsonDelta { partial_json } => {
                    if let Some(BlockState::ToolUse {
                        partial_json: buf,
                        id,
                        ..
                    }) = state.blocks.get_mut(index)
                    {
                        buf.push_str(partial_json);
                        return vec![Ok(ResponseEvent::ToolCallInputDelta {
                            item_id: id.clone(),
                            call_id: Some(id.clone()),
                            delta: partial_json.clone(),
                        })];
                    }
                }
                Delta::ThinkingDelta { thinking } => {
                    if let Some(BlockState::Thinking { buffer, .. }) = state.blocks.get_mut(index) {
                        buffer.push_str(thinking);
                    }
                    let events = vec![Ok(ResponseEvent::ReasoningContentDelta {
                        delta: thinking.clone(),
                        content_index: *index as i64,
                    })];
                    return events;
                }
                Delta::SignatureDelta { signature } => {
                    debug!(
                        "SSE signature_delta idx={} sig_len={} sig_preview={:?}",
                        index,
                        signature.len(),
                        &signature[..signature.len().min(40)]
                    );
                    match state.blocks.get_mut(index) {
                        Some(BlockState::Thinking {
                            signature: sig_slot,
                            ..
                        }) => {
                            *sig_slot = Some(signature.clone());
                        }
                        Some(BlockState::RedactedThinking {
                            signature: sig_slot,
                            ..
                        }) => {
                            *sig_slot = Some(signature.clone());
                        }
                        _ => {
                            debug!(
                                "SSE signature_delta idx={} — NO matching block state!",
                                index
                            );
                        }
                    }
                }
            }
            vec![]
        }
        SseEvent::ContentBlockStop { index } => {
            if let Some(block_state) = state.blocks.remove(index) {
                let item = match block_state {
                    BlockState::Text { buffer } => {
                        let msg = ResponseItem::Message {
                            id: None,
                            role: "assistant".into(),
                            content: vec![ContentItem::OutputText { text: buffer }],
                            phase: None,
                        };
                        return vec![Ok(ResponseEvent::provisional_output_item_done(msg))];
                    }
                    BlockState::ToolUse {
                        id,
                        name,
                        partial_json,
                    } => ResponseItem::FunctionCall {
                        id: Some(id.clone()),
                        call_id: id,
                        name,
                        arguments: partial_json,
                        namespace: None,
                    },
                    BlockState::Thinking { buffer, signature } => {
                        debug!(
                            "SSE content_block_stop thinking idx={} text_len={} has_sig={} sig_preview={:?}",
                            index,
                            buffer.len(),
                            signature.is_some(),
                            signature.as_ref().map(|s| &s[..s.len().min(40)])
                        );
                        ResponseItem::Reasoning {
                            id: format!("rs_{index}"),
                            summary: vec![],
                            content: Some(vec![ReasoningItemContent::ReasoningText {
                                text: buffer,
                            }]),
                            encrypted_content: None,
                            signature,
                        }
                    }
                    BlockState::RedactedThinking { data, signature } => {
                        debug!(
                            "SSE content_block_stop redacted_thinking idx={} data_len={} has_sig={}",
                            index,
                            data.len(),
                            signature.is_some()
                        );
                        ResponseItem::Reasoning {
                            id: format!("rs_{index}"),
                            summary: vec![],
                            content: None,
                            encrypted_content: Some(data),
                            signature,
                        }
                    }
                };
                return vec![Ok(ResponseEvent::output_item_done(item))];
            }
            vec![]
        }
        SseEvent::MessageDelta { delta, usage } => {
            let end_turn = map_stop_reason_to_end_turn(delta.stop_reason.as_deref());
            vec![Ok(ResponseEvent::Completed {
                response_id: state.response_id.clone().unwrap_or_default(),
                token_usage: Some(map_usage(usage)),
                end_turn,
            })]
        }
        SseEvent::MessageStop => vec![],
        SseEvent::Ping => vec![],
        SseEvent::Error { error } => {
            vec![Err(map_anthropic_error(&error.error_type, &error.message))]
        }
    }
}

pub(crate) fn map_stop_reason_to_end_turn(stop_reason: Option<&str>) -> Option<bool> {
    match stop_reason {
        Some("end_turn") | Some("stop_sequence") => Some(true),
        Some("max_tokens") | Some("tool_use") | Some("pause_turn") => Some(false),
        Some("refusal") => None, // Claude refused — signal as non-completion
        _ => None,
    }
}

pub(crate) fn map_usage(usage: &UsageInfo) -> TokenUsage {
    let input = usage.input_tokens.unwrap_or(0);
    let output = usage.output_tokens.unwrap_or(0);
    let cached =
        usage.cache_creation_input_tokens.unwrap_or(0) + usage.cache_read_input_tokens.unwrap_or(0);
    TokenUsage {
        input_tokens: input,
        cached_input_tokens: cached,
        output_tokens: output,
        reasoning_output_tokens: 0,
        total_tokens: input + cached + output,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use agere_api::OutputItemCompletion;

    #[test]
    fn stop_reason_end_turn_is_true() {
        assert_eq!(map_stop_reason_to_end_turn(Some("end_turn")), Some(true));
    }

    #[test]
    fn stop_reason_tool_use_is_false() {
        assert_eq!(map_stop_reason_to_end_turn(Some("tool_use")), Some(false));
    }

    #[test]
    fn stop_reason_max_tokens_is_false() {
        assert_eq!(map_stop_reason_to_end_turn(Some("max_tokens")), Some(false));
    }

    #[test]
    fn stop_reason_none_is_none() {
        assert_eq!(map_stop_reason_to_end_turn(None), None);
    }

    #[test]
    fn stop_reason_pause_turn_is_false() {
        assert_eq!(map_stop_reason_to_end_turn(Some("pause_turn")), Some(false));
    }

    #[test]
    fn stop_reason_refusal_is_none() {
        assert_eq!(map_stop_reason_to_end_turn(Some("refusal")), None);
    }

    #[test]
    fn thinking_delta_does_not_emit_summary_from_raw_thinking() {
        let mut state = SseState::new();
        let start = SseEvent::ContentBlockStart {
            index: 0,
            content_block: ContentBlockStartInfo::Thinking {
                thinking: String::new(),
            },
        };
        let _events = handle_sse_event(&start, &mut state);

        let delta = SseEvent::ContentBlockDelta {
            index: 0,
            delta: Delta::ThinkingDelta {
                thinking: "step one".to_string(),
            },
        };
        let events = handle_sse_event(&delta, &mut state);
        assert_eq!(events.len(), 1);
        assert!(
            !events
                .iter()
                .any(|event| matches!(event, Ok(ResponseEvent::ReasoningSummaryDelta { .. })))
        );
        match &events[0] {
            Ok(ResponseEvent::ReasoningContentDelta { delta, .. }) => {
                assert_eq!(delta, "step one");
            }
            other => panic!("expected ReasoningContentDelta, got {other:?}"),
        }
    }

    #[test]
    fn initial_thinking_does_not_emit_summary_from_raw_thinking() {
        let mut state = SseState::new();
        let start = SseEvent::ContentBlockStart {
            index: 0,
            content_block: ContentBlockStartInfo::Thinking {
                thinking: "seed".to_string(),
            },
        };

        let events = handle_sse_event(&start, &mut state);
        assert_eq!(events.len(), 1);
        assert!(matches!(
            &events[0],
            Ok(ResponseEvent::OutputItemAdded(
                ResponseItem::Reasoning { .. }
            ))
        ));
        assert!(
            !events
                .iter()
                .any(|event| matches!(event, Ok(ResponseEvent::ReasoningSummaryDelta { .. })))
        );
    }

    #[test]
    fn thinking_done_does_not_include_summary_from_raw_thinking() {
        let mut state = SseState::new();
        let start = SseEvent::ContentBlockStart {
            index: 0,
            content_block: ContentBlockStartInfo::Thinking {
                thinking: "seed".to_string(),
            },
        };
        let _events = handle_sse_event(&start, &mut state);
        let stop = SseEvent::ContentBlockStop { index: 0 };
        let events = handle_sse_event(&stop, &mut state);
        match &events[0] {
            Ok(ResponseEvent::OutputItemDone { item, .. }) => match item {
                ResponseItem::Reasoning {
                    summary, content, ..
                } => {
                    assert!(summary.is_empty());
                    assert!(matches!(
                        content.as_deref(),
                        Some([ReasoningItemContent::ReasoningText { text }]) if text == "seed"
                    ));
                }
                other => panic!("expected Reasoning, got {other:?}"),
            },
            other => panic!("expected OutputItemDone, got {other:?}"),
        }
    }

    #[test]
    fn usage_adds_cache_tokens() {
        let usage = UsageInfo {
            input_tokens: Some(100),
            output_tokens: Some(50),
            cache_creation_input_tokens: Some(10),
            cache_read_input_tokens: Some(40),
        };
        let result = map_usage(&usage);
        assert_eq!(result.input_tokens, 100);
        assert_eq!(result.output_tokens, 50);
        assert_eq!(result.cached_input_tokens, 50); // 10+40
        assert_eq!(result.total_tokens, 200);
    }

    #[test]
    fn usage_handles_null_tokens() {
        let usage = UsageInfo {
            input_tokens: None,
            output_tokens: None,
            cache_creation_input_tokens: None,
            cache_read_input_tokens: None,
        };
        let result = map_usage(&usage);
        assert_eq!(result.input_tokens, 0);
        assert_eq!(result.output_tokens, 0);
        assert_eq!(result.cached_input_tokens, 0);
    }

    #[test]
    fn message_start_event_returns_none() {
        let event = SseEvent::MessageStart {
            message: MessageStartInfo {
                id: "msg_1".into(),
                model: "claude-1".into(),
                usage: UsageInfo {
                    input_tokens: Some(1),
                    output_tokens: Some(0),
                    cache_creation_input_tokens: None,
                    cache_read_input_tokens: None,
                },
            },
        };
        let result = handle_sse_event(&event, &mut SseState::new());
        assert!(result.is_empty());
    }

    #[test]
    fn text_delta_emits_output_text_delta() {
        let mut state = SseState::new();
        state.ensure_block(0, &ContentBlockStartInfo::Text { text: "".into() });

        let event = SseEvent::ContentBlockDelta {
            index: 0,
            delta: Delta::TextDelta {
                text: "Hello".into(),
            },
        };
        let result = handle_sse_event(&event, &mut state);
        assert_eq!(result.len(), 1);
        match &result[0] {
            Ok(ResponseEvent::OutputTextDelta(text)) => assert_eq!(text, "Hello"),
            other => panic!("expected OutputTextDelta, got {other:?}"),
        }
    }

    #[test]
    fn content_block_stop_emits_provisional_output_item_done_for_text() {
        let mut state = SseState::new();
        state.ensure_block(0, &ContentBlockStartInfo::Text { text: "".into() });
        let _ = handle_sse_event(
            &SseEvent::ContentBlockDelta {
                index: 0,
                delta: Delta::TextDelta { text: "Hi".into() },
            },
            &mut state,
        );

        // ContentBlockStop for text emits immediate Provisional OutputItemDone
        let result = handle_sse_event(&SseEvent::ContentBlockStop { index: 0 }, &mut state);
        assert_eq!(result.len(), 1);
        match &result[0] {
            Ok(ResponseEvent::OutputItemDone { item, completion }) => {
                assert_eq!(*completion, OutputItemCompletion::Provisional);
                match item {
                    ResponseItem::Message { role, content, .. } => {
                        assert_eq!(role, "assistant");
                        assert_eq!(content.len(), 1);
                        assert_eq!(content[0], ContentItem::OutputText { text: "Hi".into() });
                    }
                    _ => panic!("expected Message item"),
                }
            }
            other => panic!("expected OutputItemDone, got {other:?}"),
        }

        // MessageDelta emits only Completed (no buffered message)
        let result = handle_sse_event(
            &SseEvent::MessageDelta {
                delta: crate::types::MessageDeltaInfo {
                    stop_reason: Some("end_turn".into()),
                    stop_sequence: None,
                },
                usage: crate::types::UsageInfo {
                    input_tokens: Some(10),
                    output_tokens: Some(3),
                    cache_creation_input_tokens: None,
                    cache_read_input_tokens: None,
                },
            },
            &mut state,
        );
        assert_eq!(result.len(), 1);
        assert!(matches!(&result[0], Ok(ResponseEvent::Completed { .. })));
    }

    #[test]
    fn content_block_stop_emits_final_for_tool_use() {
        let mut state = SseState::new();
        state.ensure_block(
            0,
            &ContentBlockStartInfo::ToolUse {
                id: "tool-1".into(),
                name: "my_tool".into(),
            },
        );

        let _ = handle_sse_event(
            &SseEvent::ContentBlockDelta {
                index: 0,
                delta: Delta::InputJsonDelta {
                    partial_json: r#"{"arg":1}"#.into(),
                },
            },
            &mut state,
        );

        let result = handle_sse_event(&SseEvent::ContentBlockStop { index: 0 }, &mut state);
        assert_eq!(result.len(), 1);

        match &result[0] {
            Ok(ResponseEvent::OutputItemDone { item, completion }) => {
                assert_eq!(*completion, OutputItemCompletion::Final);
                match item {
                    ResponseItem::FunctionCall {
                        name, arguments, ..
                    } => {
                        assert_eq!(name, "my_tool");
                        assert_eq!(arguments, r#"{"arg":1}"#);
                    }
                    _ => panic!("expected FunctionCall item"),
                }
            }
            other => panic!("expected OutputItemDone, got {other:?}"),
        }
    }
}
