use crate::types::ChatDeltaToolCall;
use crate::types::ChatSseEvent;
use agere_api::ApiError;
use agere_api::ResponseEvent;
use agere_protocol::models::ContentItem;
use agere_protocol::models::ReasoningItemContent;
use agere_protocol::models::ResponseItem;
use agere_protocol::protocol::TokenUsage;
use std::collections::HashMap;
use tracing::debug;

/// State for processing Chat Completions SSE stream.
#[derive(Debug)]
pub(crate) struct ChatSseState {
    pub response_id: Option<String>,
    pub server_model: Option<String>,
    pub tool_calls: HashMap<usize, ToolCallAccumulator>,
    pub pending_completion: Option<PendingCompletion>,
    /// Tracks whether we already emitted OutputItemAdded for the current assistant message.
    /// Reset to false after each Completed event so the next turn can emit a new message.
    pub assistant_message_emitted: bool,
    /// Accumulates assistant text content across OutputTextDelta events.
    /// Cleared on each OutputItemAdded and consumed by OutputItemDone.
    pub assistant_text_buffer: String,
    /// Accumulates assistant reasoning content.
    pub assistant_reasoning_buffer: String,
    /// Tracks whether we already emitted OutputItemAdded for the current reasoning item.
    pub assistant_reasoning_emitted: bool,
    /// Tracks whether a terminal completion event has already been emitted.
    pub completed: bool,
}

#[derive(Debug)]
pub(crate) struct ToolCallAccumulator {
    pub id: String,
    #[allow(dead_code)]
    pub name: String,
    pub arguments: String,
}

#[derive(Debug)]
pub(crate) struct PendingCompletion {
    pub end_turn: Option<bool>,
}

impl ChatSseState {
    pub fn new() -> Self {
        Self {
            response_id: None,
            server_model: None,
            tool_calls: HashMap::new(),
            pending_completion: None,
            assistant_message_emitted: false,
            assistant_text_buffer: String::new(),
            assistant_reasoning_buffer: String::new(),
            assistant_reasoning_emitted: false,
            completed: false,
        }
    }
}

impl Default for ChatSseState {
    fn default() -> Self {
        Self::new()
    }
}

/// Handle a single parsed SSE event, returning zero or more ResponseEvents.
pub(crate) fn handle_chat_sse_event(
    event: &ChatSseEvent,
    state: &mut ChatSseState,
) -> Vec<Result<ResponseEvent, ApiError>> {
    let mut results = Vec::new();

    match event {
        ChatSseEvent::Done => {
            debug!("chat_sse: [DONE]");
            emit_pending_completion(state, None, &mut results);
        }
        ChatSseEvent::Chunk {
            id,
            model,
            choices,
            usage,
        } => {
            debug!(
                "chat_sse: chunk id={} model={} choices={} usage={}",
                id,
                model,
                choices.len(),
                usage.is_some()
            );

            // Store response ID and model on first chunk
            if state.response_id.is_none() && !id.is_empty() {
                state.response_id = Some(id.clone());
            }
            if state.server_model.is_none() && !model.is_empty() {
                state.server_model = Some(model.clone());
            }

            if choices.is_empty()
                && let Some(usage) = usage.as_ref()
            {
                emit_pending_completion(state, Some(map_usage(usage)), &mut results);
            }

            for choice in choices {
                let delta = &choice.delta;

                // Role delta: emit OutputItemAdded for assistant messages
                // (must happen before OutputTextDelta so the core has an active item).
                if let Some(role) = &delta.role
                    && role == "assistant"
                    && !state.assistant_message_emitted
                    && !state.assistant_reasoning_emitted
                    && state.assistant_text_buffer.is_empty()
                    && state.assistant_reasoning_buffer.is_empty()
                {
                    state.assistant_text_buffer.clear();
                    state.assistant_reasoning_buffer.clear();
                    state.assistant_message_emitted = false;
                    state.assistant_reasoning_emitted = false;
                }

                // Reasoning content delta
                if let Some(reasoning) = &delta.reasoning
                    && !reasoning.is_empty()
                {
                    ensure_reasoning_item(state, &mut results);
                    state.assistant_reasoning_buffer.push_str(reasoning);
                    debug!("chat_sse: ReasoningContentDelta len={}", reasoning.len());
                    results.push(Ok(ResponseEvent::ReasoningContentDelta {
                        delta: reasoning.clone(),
                        content_index: 0,
                    }));
                }

                // Text content delta
                if let Some(text) = &delta.content
                    && !text.is_empty()
                {
                    finish_reasoning_item(state, &mut results);
                    ensure_message_item(state, &mut results);
                    state.assistant_text_buffer.push_str(text);
                    debug!("chat_sse: OutputTextDelta len={}", text.len());
                    results.push(Ok(ResponseEvent::OutputTextDelta(text.clone())));
                }

                // Tool call deltas
                if let Some(tool_calls) = &delta.tool_calls {
                    debug!("chat_sse: {} tool_calls in delta", tool_calls.len());
                    if !tool_calls.is_empty() {
                        finish_reasoning_item(state, &mut results);
                    }
                    for tc in tool_calls {
                        process_tool_call_delta(tc, state, &mut results);
                    }
                }

                // Finish reason — skip empty string (some APIs return "" for "still streaming")
                if let Some(finish_reason) = &choice.finish_reason
                    && !finish_reason.is_empty()
                {
                    debug!("chat_sse: finish_reason={}", finish_reason);
                    let end_turn = map_finish_reason_to_end_turn(finish_reason);

                    // 1. Finish any remaining active reasoning item. Streams that begin tool
                    //    calls finish reasoning before OutputItemAdded(FunctionCall), but this
                    //    still handles reasoning-only streams that end without text.
                    finish_reasoning_item(state, &mut results);

                    // 2. Emit OutputItemDone(FunctionCall) for each accumulated tool call.
                    //    This is CRITICAL: the core session (turn.rs) dispatches tool calls
                    //    only upon receiving OutputItemDone(FunctionCall). Without it, the
                    //    TUI never shows "Working" and the tool is never executed.
                    //    The accumulated arguments are the fully streamed JSON string.
                    let tool_call_ids: Vec<usize> = state.tool_calls.keys().copied().collect();
                    for idx in tool_call_ids {
                        if let Some(acc) = state.tool_calls.remove(&idx) {
                            debug!(
                                "chat_sse: emitting OutputItemDone(FunctionCall) idx={} id={} name={} args_len={}",
                                idx,
                                acc.id,
                                acc.name,
                                acc.arguments.len()
                            );
                            let item = ResponseItem::FunctionCall {
                                id: None,
                                call_id: acc.id.clone(),
                                name: acc.name.clone(),
                                arguments: acc.arguments.clone(),
                                namespace: None,
                            };
                            results.push(Ok(ResponseEvent::output_item_done(item)));
                        }
                    }

                    // 3. Emit OutputItemDone(Message) for accumulated text content (if any).
                    //    Record the assistant's text into the conversation history
                    //    for continuity on the next turn.
                    let mut content_items: Vec<ContentItem> = Vec::new();
                    if !state.assistant_text_buffer.is_empty() {
                        content_items.push(ContentItem::OutputText {
                            text: std::mem::take(&mut state.assistant_text_buffer),
                        });
                    }
                    let done_msg = ResponseItem::Message {
                        id: None,
                        role: "assistant".into(),
                        content: content_items,
                        phase: None,
                    };
                    debug!(
                        "chat_sse: emitting OutputItemDone(Message) text_len={}",
                        done_item_content_len(&done_msg)
                    );
                    results.push(Ok(ResponseEvent::output_item_done(done_msg)));

                    state.assistant_message_emitted = false;
                    state.assistant_reasoning_emitted = false;
                    state.tool_calls.clear();
                    if let Some(usage) = usage.as_ref() {
                        emit_completed_event(state, Some(map_usage(usage)), end_turn, &mut results);
                    } else {
                        state.pending_completion = Some(PendingCompletion { end_turn });
                    }
                }
            }
        }
    }

    results
}

fn emit_pending_completion(
    state: &mut ChatSseState,
    token_usage: Option<TokenUsage>,
    results: &mut Vec<Result<ResponseEvent, ApiError>>,
) {
    if let Some(pending) = state.pending_completion.take() {
        emit_completed_event(state, token_usage, pending.end_turn, results);
    }
}

fn emit_completed_event(
    state: &mut ChatSseState,
    token_usage: Option<TokenUsage>,
    end_turn: Option<bool>,
    results: &mut Vec<Result<ResponseEvent, ApiError>>,
) {
    state.completed = true;
    results.push(Ok(ResponseEvent::Completed {
        response_id: state.response_id.clone().unwrap_or_default(),
        token_usage,
        end_turn,
    }));
}

fn ensure_message_item(
    state: &mut ChatSseState,
    results: &mut Vec<Result<ResponseEvent, ApiError>>,
) {
    if state.assistant_message_emitted {
        return;
    }
    state.assistant_message_emitted = true;
    debug!("chat_sse: emitting OutputItemAdded(Message assistant)");
    let item = ResponseItem::Message {
        id: None,
        role: "assistant".into(),
        content: vec![],
        phase: None,
    };
    results.push(Ok(ResponseEvent::OutputItemAdded(item)));
}

fn ensure_reasoning_item(
    state: &mut ChatSseState,
    results: &mut Vec<Result<ResponseEvent, ApiError>>,
) {
    if state.assistant_reasoning_emitted {
        return;
    }
    state.assistant_reasoning_emitted = true;
    debug!("chat_sse: emitting OutputItemAdded(Reasoning)");
    let item = ResponseItem::Reasoning {
        id: "rs_chat_0".to_string(),
        summary: vec![],
        content: Some(vec![]),
        encrypted_content: None,
        signature: None,
    };
    results.push(Ok(ResponseEvent::OutputItemAdded(item)));
}

fn finish_reasoning_item(
    state: &mut ChatSseState,
    results: &mut Vec<Result<ResponseEvent, ApiError>>,
) {
    if !state.assistant_reasoning_emitted {
        return;
    }
    let text = std::mem::take(&mut state.assistant_reasoning_buffer);
    let content = (!text.is_empty()).then(|| vec![ReasoningItemContent::ReasoningText { text }]);
    let item = ResponseItem::Reasoning {
        id: "rs_chat_0".to_string(),
        summary: vec![],
        content,
        encrypted_content: None,
        signature: None,
    };
    results.push(Ok(ResponseEvent::output_item_done(item)));
    state.assistant_reasoning_emitted = false;
}

fn done_item_content_len(item: &ResponseItem) -> usize {
    match item {
        ResponseItem::Message { content, .. } => content
            .iter()
            .map(|c| match c {
                ContentItem::InputText { text } | ContentItem::OutputText { text } => text.len(),
                _ => 0,
            })
            .sum(),
        _ => 0,
    }
}

fn process_tool_call_delta(
    tc: &ChatDeltaToolCall,
    state: &mut ChatSseState,
    results: &mut Vec<Result<ResponseEvent, ApiError>>,
) {
    let idx = tc.index;

    // New tool call: has non-empty id and name in this chunk.
    // OpenAI streaming sends id/name only in the first chunk, then sends ""
    // in subsequent chunks — we must not re-insert or re-emit.
    if let (Some(id), Some(func)) = (&tc.id, &tc.function)
        && !id.is_empty()
        && let Some(name) = &func.name
        && !name.is_empty()
    {
        // Only emit if not already known
        if state.tool_calls.contains_key(&idx) {
            return;
        }

        state.tool_calls.insert(
            idx,
            ToolCallAccumulator {
                id: id.clone(),
                name: name.clone(),
                arguments: String::new(),
            },
        );

        debug!(
            "chat_sse: new tool_call idx={} id={} name={}",
            idx, id, name
        );

        let item = ResponseItem::FunctionCall {
            id: Some(id.clone()),
            call_id: id.clone(),
            name: name.clone(),
            arguments: String::new(),
            namespace: None,
        };
        results.push(Ok(ResponseEvent::OutputItemAdded(item)));
    }

    // Argument delta
    if let Some(func) = &tc.function
        && let Some(args) = &func.arguments
    {
        if let Some(accumulator) = state.tool_calls.get_mut(&idx) {
            accumulator.arguments.push_str(args);
            debug!(
                "chat_sse: tool_arg_delta idx={} args_len={}",
                idx,
                accumulator.arguments.len()
            );

            let call_id = accumulator.id.clone();
            results.push(Ok(ResponseEvent::ToolCallInputDelta {
                item_id: call_id.clone(),
                call_id: Some(call_id),
                delta: args.clone(),
            }));
        } else {
            debug!("chat_sse: tool_arg_delta idx={} NO ACCUMULATOR", idx);
        }
    }
}

pub(crate) fn map_finish_reason_to_end_turn(finish_reason: &str) -> Option<bool> {
    match finish_reason {
        "stop" => Some(true),
        "tool_calls" | "length" => Some(false),
        "content_filter" => None,
        _ => None,
    }
}

pub(crate) fn map_usage(usage: &crate::types::ChatUsage) -> TokenUsage {
    let prompt = usage.prompt_tokens.unwrap_or(0);
    let completion = usage.completion_tokens.unwrap_or(0);
    let cached = usage
        .prompt_tokens_details
        .as_ref()
        .and_then(|d| d.cached_tokens)
        .unwrap_or(0);
    let total = usage.total_tokens.unwrap_or(prompt + completion);
    TokenUsage {
        input_tokens: prompt,
        cached_input_tokens: cached,
        output_tokens: completion,
        reasoning_output_tokens: 0,
        total_tokens: total,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use agere_api::ResponseEvent;

    #[test]
    fn text_delta_emits_output_text_delta() {
        let json = r#"{"id":"chatcmpl-123","object":"chat.completion.chunk","created":1234567890,"model":"gpt-4o","choices":[{"index":0,"delta":{"role":"assistant","content":"Hello"},"finish_reason":null}]}"#;
        let mut state = ChatSseState::new();
        let events = handle_chat_sse_event(&serde_json::from_str(json).unwrap(), &mut state);
        assert_eq!(events.len(), 2);
        // First event: OutputItemAdded (message)
        match &events[0] {
            Ok(ResponseEvent::OutputItemAdded(item)) => match item {
                agere_protocol::models::ResponseItem::Message { role, content, .. } => {
                    assert_eq!(role, "assistant");
                    assert!(content.is_empty());
                }
                other => panic!("expected Message, got {other:?}"),
            },
            other => panic!("expected OutputItemAdded, got {other:?}"),
        }
        // Second event: OutputTextDelta
        match &events[1] {
            Ok(ResponseEvent::OutputTextDelta(text)) => assert_eq!(text, "Hello"),
            other => panic!("expected OutputTextDelta, got {other:?}"),
        }
    }

    #[test]
    fn reasoning_delta_emits_reasoning_content_delta() {
        let json = r#"{"id":"chatcmpl-123","object":"chat.completion.chunk","created":1234567890,"model":"gpt-4o","choices":[{"index":0,"delta":{"reasoning":"Let me think about this"},"finish_reason":null}]}"#;
        let mut state = ChatSseState::new();
        let events = handle_chat_sse_event(&serde_json::from_str(json).unwrap(), &mut state);
        assert_eq!(events.len(), 2);
        match &events[0] {
            Ok(ResponseEvent::OutputItemAdded(ResponseItem::Reasoning { .. })) => {}
            other => panic!("expected OutputItemAdded(Reasoning), got {other:?}"),
        }
        match &events[1] {
            Ok(ResponseEvent::ReasoningContentDelta { delta, .. }) => {
                assert_eq!(delta, "Let me think about this");
            }
            other => panic!("expected ReasoningContentDelta, got {other:?}"),
        }
    }

    #[test]
    fn reasoning_delta_does_not_emit_summary_from_raw_reasoning() {
        let json = r#"{"id":"chatcmpl-123","object":"chat.completion.chunk","created":1234567890,"model":"gpt-4o","choices":[{"index":0,"delta":{"reasoning":"Let me think"},"finish_reason":null}]}"#;
        let mut state = ChatSseState::new();
        let events = handle_chat_sse_event(&serde_json::from_str(json).unwrap(), &mut state);
        assert_eq!(events.len(), 2);
        assert!(
            !events
                .iter()
                .any(|event| matches!(event, Ok(ResponseEvent::ReasoningSummaryDelta { .. })))
        );
        match &events[1] {
            Ok(ResponseEvent::ReasoningContentDelta { delta, .. }) => {
                assert_eq!(delta, "Let me think");
            }
            other => panic!("expected ReasoningContentDelta, got {other:?}"),
        }
    }

    #[test]
    fn mixed_reasoning_and_text_chunk_finishes_reasoning_before_text() {
        let mixed = r#"{"id":"chatcmpl-123","object":"chat.completion.chunk","created":1234567890,"model":"gpt-4o","choices":[{"index":0,"delta":{"reasoning":"think","content":"answer"},"finish_reason":null}]}"#;
        let next_text = r#"{"id":"chatcmpl-123","object":"chat.completion.chunk","created":1234567890,"model":"gpt-4o","choices":[{"index":0,"delta":{"content":" more"},"finish_reason":null}]}"#;

        let mut state = ChatSseState::new();
        let events0 = handle_chat_sse_event(&serde_json::from_str(mixed).unwrap(), &mut state);
        assert!(matches!(
            &events0[0],
            Ok(ResponseEvent::OutputItemAdded(
                ResponseItem::Reasoning { .. }
            ))
        ));
        assert!(matches!(
            &events0[1],
            Ok(ResponseEvent::ReasoningContentDelta { .. })
        ));
        assert!(matches!(
            &events0[2],
            Ok(ResponseEvent::OutputItemDone {
                item: ResponseItem::Reasoning { .. },
                ..
            })
        ));
        assert!(matches!(
            &events0[3],
            Ok(ResponseEvent::OutputItemAdded(ResponseItem::Message { .. }))
        ));
        assert!(
            matches!(&events0[4], Ok(ResponseEvent::OutputTextDelta(text)) if text == "answer")
        );

        let events1 = handle_chat_sse_event(&serde_json::from_str(next_text).unwrap(), &mut state);
        assert!(matches!(&events1[0], Ok(ResponseEvent::OutputTextDelta(text)) if text == " more"));
    }

    #[test]
    fn finish_reason_emits_reasoning_done_separate_from_message() {
        let reasoning = r#"{"id":"chatcmpl-123","object":"chat.completion.chunk","created":1234567890,"model":"gpt-4o","choices":[{"index":0,"delta":{"reasoning":"think"},"finish_reason":null}]}"#;
        let text = r#"{"id":"chatcmpl-123","object":"chat.completion.chunk","created":1234567890,"model":"gpt-4o","choices":[{"index":0,"delta":{"content":"answer"},"finish_reason":null}]}"#;
        let finish = r#"{"id":"chatcmpl-123","object":"chat.completion.chunk","created":1234567890,"model":"gpt-4o","choices":[{"index":0,"delta":{},"finish_reason":"stop"}]}"#;

        let mut state = ChatSseState::new();
        let _events0 = handle_chat_sse_event(&serde_json::from_str(reasoning).unwrap(), &mut state);
        let events1 = handle_chat_sse_event(&serde_json::from_str(text).unwrap(), &mut state);
        assert!(matches!(
            &events1[0],
            Ok(ResponseEvent::OutputItemDone {
                item: ResponseItem::Reasoning { .. },
                ..
            })
        ));
        assert!(matches!(
            &events1[1],
            Ok(ResponseEvent::OutputItemAdded(ResponseItem::Message { .. }))
        ));

        let events2 = handle_chat_sse_event(&serde_json::from_str(finish).unwrap(), &mut state);
        match &events2[0] {
            Ok(ResponseEvent::OutputItemDone { item, .. }) => match item {
                ResponseItem::Message { content, .. } => {
                    assert_eq!(content.len(), 1);
                    assert!(
                        matches!(&content[0], ContentItem::OutputText { text } if text == "answer")
                    );
                }
                other => panic!("expected Message, got {other:?}"),
            },
            other => panic!("expected OutputItemDone(Message), got {other:?}"),
        }
    }

    #[test]
    fn tool_call_start_finishes_active_reasoning_before_function_call() {
        let reasoning = r#"{"id":"chatcmpl-123","object":"chat.completion.chunk","created":1234567890,"model":"gpt-4o","choices":[{"index":0,"delta":{"reasoning":"think"},"finish_reason":null}]}"#;
        let tool_start = r#"{"id":"chatcmpl-123","object":"chat.completion.chunk","created":1234567890,"model":"gpt-4o","choices":[{"index":0,"delta":{"tool_calls":[{"index":0,"id":"call_abc","type":"function","function":{"name":"get_weather"}}]},"finish_reason":null}]}"#;
        let tool_args = r#"{"id":"chatcmpl-123","object":"chat.completion.chunk","created":1234567890,"model":"gpt-4o","choices":[{"index":0,"delta":{"tool_calls":[{"index":0,"function":{"arguments":"{\"city\":\"NYC\"}"}}]},"finish_reason":null}]}"#;
        let finish = r#"{"id":"chatcmpl-123","object":"chat.completion.chunk","created":1234567890,"model":"gpt-4o","choices":[{"index":0,"delta":{},"finish_reason":"tool_calls"}]}"#;

        let mut state = ChatSseState::new();
        let _events0 = handle_chat_sse_event(&serde_json::from_str(reasoning).unwrap(), &mut state);
        let events1 = handle_chat_sse_event(&serde_json::from_str(tool_start).unwrap(), &mut state);

        assert!(matches!(
            &events1[0],
            Ok(ResponseEvent::OutputItemDone {
                item: ResponseItem::Reasoning { .. },
                ..
            })
        ));
        assert!(matches!(
            &events1[1],
            Ok(ResponseEvent::OutputItemAdded(
                ResponseItem::FunctionCall { .. }
            ))
        ));

        let _events2 = handle_chat_sse_event(&serde_json::from_str(tool_args).unwrap(), &mut state);
        let events3 = handle_chat_sse_event(&serde_json::from_str(finish).unwrap(), &mut state);

        assert!(matches!(
            &events3[0],
            Ok(ResponseEvent::OutputItemDone {
                item: ResponseItem::FunctionCall { .. },
                ..
            })
        ));
    }

    #[test]
    fn tool_call_start_emits_output_item_added() {
        let json = r#"{"id":"chatcmpl-123","object":"chat.completion.chunk","created":1234567890,"model":"gpt-4o","choices":[{"index":0,"delta":{"role":"assistant","tool_calls":[{"index":0,"id":"call_abc","type":"function","function":{"name":"get_weather"}}]},"finish_reason":null}]}"#;
        let mut state = ChatSseState::new();
        let events = handle_chat_sse_event(&serde_json::from_str(json).unwrap(), &mut state);
        assert_eq!(events.len(), 1);
        match &events[0] {
            Ok(ResponseEvent::OutputItemAdded(item)) => match item {
                agere_protocol::models::ResponseItem::FunctionCall { call_id, name, .. } => {
                    assert_eq!(call_id, "call_abc");
                    assert_eq!(name, "get_weather");
                }
                other => panic!("expected FunctionCall, got {other:?}"),
            },
            other => panic!("expected OutputItemAdded(FunctionCall), got {other:?}"),
        }
    }

    #[test]
    fn tool_call_arguments_accumulated() {
        let mut state = ChatSseState::new();

        // First: tool call start (no role field, so only FunctionCall OutputItemAdded)
        let start = r#"{"id":"chatcmpl-123","object":"chat.completion.chunk","created":1234567890,"model":"gpt-4o","choices":[{"index":0,"delta":{"tool_calls":[{"index":0,"id":"call_abc","type":"function","function":{"name":"get_weather"}}]},"finish_reason":null}]}"#;
        let events0 = handle_chat_sse_event(&serde_json::from_str(start).unwrap(), &mut state);
        assert_eq!(events0.len(), 1);
        assert!(matches!(&events0[0], Ok(ResponseEvent::OutputItemAdded(_))));

        // Second: argument delta part 1
        let arg1 = r#"{"id":"chatcmpl-123","object":"chat.completion.chunk","created":1234567890,"model":"gpt-4o","choices":[{"index":0,"delta":{"tool_calls":[{"index":0,"function":{"arguments":"{\"city\":"}}]},"finish_reason":null}]}"#;
        let events1 = handle_chat_sse_event(&serde_json::from_str(arg1).unwrap(), &mut state);
        assert_eq!(events1.len(), 1);
        match &events1[0] {
            Ok(ResponseEvent::ToolCallInputDelta { delta, .. }) => {
                assert_eq!(delta, r#"{"city":"#);
            }
            other => panic!("expected ToolCallInputDelta, got {other:?}"),
        }

        // Third: argument delta part 2
        let arg2 = r#"{"id":"chatcmpl-123","object":"chat.completion.chunk","created":1234567890,"model":"gpt-4o","choices":[{"index":0,"delta":{"tool_calls":[{"index":0,"function":{"arguments":"\"NYC\"}"}}]},"finish_reason":null}]}"#;
        let events2 = handle_chat_sse_event(&serde_json::from_str(arg2).unwrap(), &mut state);
        match &events2[0] {
            Ok(ResponseEvent::ToolCallInputDelta { delta, .. }) => {
                assert_eq!(delta, r#""NYC"}"#);
            }
            other => panic!("expected ToolCallInputDelta, got {other:?}"),
        }
    }

    #[test]
    fn tool_call_empty_id_does_not_re_emit_output_item_added() {
        // After the initial tool call with id, subsequent chunks send empty id.
        // These should NOT re-emit OutputItemAdded.
        let mut state = ChatSseState::new();

        // First: tool call start with id and name
        let start = r#"{"id":"chatcmpl-123","object":"chat.completion.chunk","created":1234567890,"model":"gpt-4o","choices":[{"index":0,"delta":{"tool_calls":[{"index":0,"id":"call_abc","type":"function","function":{"name":"get_weather"}}]},"finish_reason":null}]}"#;
        let events0 = handle_chat_sse_event(&serde_json::from_str(start).unwrap(), &mut state);
        assert_eq!(events0.len(), 1);
        assert!(matches!(&events0[0], Ok(ResponseEvent::OutputItemAdded(_))));

        // Second: chunk with empty id and empty name — should only emit ToolCallInputDelta
        let empty = r#"{"id":"chatcmpl-123","object":"chat.completion.chunk","created":1234567890,"model":"gpt-4o","choices":[{"index":0,"delta":{"tool_calls":[{"index":0,"id":"","function":{"name":"","arguments":"{\"city\":\""}}]},"finish_reason":null}]}"#;
        let events1 = handle_chat_sse_event(&serde_json::from_str(empty).unwrap(), &mut state);
        assert_eq!(events1.len(), 1);
        assert!(
            matches!(&events1[0], Ok(ResponseEvent::ToolCallInputDelta { .. })),
            "expected ToolCallInputDelta, not OutputItemAdded: {:?}",
            events1[0]
        );
    }

    #[test]
    fn finish_reason_stop_emits_completed_with_end_turn_true() {
        let json = r#"{"id":"chatcmpl-123","object":"chat.completion.chunk","created":1234567890,"model":"gpt-4o","choices":[{"index":0,"delta":{},"finish_reason":"stop"}],"usage":{"prompt_tokens":10,"completion_tokens":5,"total_tokens":15}}"#;
        let mut state = ChatSseState::new();
        let events = handle_chat_sse_event(&serde_json::from_str(json).unwrap(), &mut state);
        // OutputItemDone (with empty content) + Completed
        assert_eq!(events.len(), 2);
        match &events[0] {
            Ok(ResponseEvent::OutputItemDone { item, .. }) => match item {
                ResponseItem::Message { role, content, .. } => {
                    assert_eq!(role, "assistant");
                    assert!(content.is_empty());
                }
                other => panic!("expected Message, got {other:?}"),
            },
            other => panic!("expected OutputItemDone, got {other:?}"),
        }
        match &events[1] {
            Ok(ResponseEvent::Completed {
                end_turn,
                token_usage,
                ..
            }) => {
                assert_eq!(end_turn, &Some(true));
                assert!(token_usage.is_some());
            }
            other => panic!("expected Completed, got {other:?}"),
        }
    }

    #[test]
    fn usage_only_chunk_after_finish_emits_completed_with_usage() {
        let finish = r#"{"id":"chatcmpl-123","object":"chat.completion.chunk","created":1234567890,"model":"gpt-4o","choices":[{"index":0,"delta":{},"finish_reason":"stop"}]}"#;
        let usage = r#"{"id":"chatcmpl-123","object":"chat.completion.chunk","created":1234567890,"model":"gpt-4o","choices":[],"usage":{"prompt_tokens":10,"completion_tokens":5,"total_tokens":15}}"#;

        let mut state = ChatSseState::new();
        let finish_events =
            handle_chat_sse_event(&serde_json::from_str(finish).unwrap(), &mut state);
        assert!(
            !finish_events
                .iter()
                .any(|event| matches!(event, Ok(ResponseEvent::Completed { .. })))
        );

        let usage_events = handle_chat_sse_event(&serde_json::from_str(usage).unwrap(), &mut state);
        assert!(matches!(
            &usage_events[..],
            [Ok(ResponseEvent::Completed {
                end_turn: Some(true),
                token_usage: Some(TokenUsage {
                    input_tokens: 10,
                    output_tokens: 5,
                    total_tokens: 15,
                    ..
                }),
                ..
            })]
        ));
    }

    #[test]
    fn finish_reason_emits_output_item_done_with_accumulated_text() {
        // Simulate a full turn: first chunk with role, text deltas, then finish_reason
        let chunk1 = r#"{"id":"chatcmpl-123","object":"chat.completion.chunk","created":1234567890,"model":"gpt-4o","choices":[{"index":0,"delta":{"role":"assistant","content":"Hello"},"finish_reason":null}]}"#;
        let chunk2 = r#"{"id":"chatcmpl-123","object":"chat.completion.chunk","created":1234567890,"model":"gpt-4o","choices":[{"index":0,"delta":{"content":" World"},"finish_reason":null}]}"#;
        let chunk3 = r#"{"id":"chatcmpl-123","object":"chat.completion.chunk","created":1234567890,"model":"gpt-4o","choices":[{"index":0,"delta":{},"finish_reason":"stop"}],"usage":{"prompt_tokens":10,"completion_tokens":5,"total_tokens":15}}"#;

        let mut state = ChatSseState::new();
        let _events1 = handle_chat_sse_event(&serde_json::from_str(chunk1).unwrap(), &mut state);
        let _events2 = handle_chat_sse_event(&serde_json::from_str(chunk2).unwrap(), &mut state);
        let events3 = handle_chat_sse_event(&serde_json::from_str(chunk3).unwrap(), &mut state);

        // events3[0] should be OutputItemDone with accumulated "Hello World"
        match &events3[0] {
            Ok(ResponseEvent::OutputItemDone { item, .. }) => match item {
                ResponseItem::Message { content, .. } => {
                    assert_eq!(content.len(), 1);
                    match &content[0] {
                        ContentItem::OutputText { text } => assert_eq!(text, "Hello World"),
                        other => panic!("expected OutputText, got {other:?}"),
                    }
                }
                other => panic!("expected Message, got {other:?}"),
            },
            other => panic!("expected OutputItemDone, got {other:?}"),
        }
    }

    #[test]
    fn repeated_assistant_role_preserves_accumulated_text() {
        let chunk1 = r#"{"id":"chatcmpl-123","object":"chat.completion.chunk","created":1234567890,"model":"gpt-4o","choices":[{"index":0,"delta":{"role":"assistant","content":"Hello"},"finish_reason":null}]}"#;
        let chunk2 = r#"{"id":"chatcmpl-123","object":"chat.completion.chunk","created":1234567890,"model":"gpt-4o","choices":[{"index":0,"delta":{"role":"assistant","content":" World"},"finish_reason":null}]}"#;
        let finish = r#"{"id":"chatcmpl-123","object":"chat.completion.chunk","created":1234567890,"model":"gpt-4o","choices":[{"index":0,"delta":{},"finish_reason":"stop"}]}"#;

        let mut state = ChatSseState::new();
        let _events1 = handle_chat_sse_event(&serde_json::from_str(chunk1).unwrap(), &mut state);
        let _events2 = handle_chat_sse_event(&serde_json::from_str(chunk2).unwrap(), &mut state);
        let events3 = handle_chat_sse_event(&serde_json::from_str(finish).unwrap(), &mut state);

        match &events3[0] {
            Ok(ResponseEvent::OutputItemDone { item, .. }) => match item {
                ResponseItem::Message { content, .. } => {
                    assert_eq!(content.len(), 1);
                    assert!(matches!(
                        &content[0],
                        ContentItem::OutputText { text } if text == "Hello World"
                    ));
                }
                other => panic!("expected Message, got {other:?}"),
            },
            other => panic!("expected OutputItemDone, got {other:?}"),
        }
    }

    #[test]
    fn repeated_assistant_role_preserves_accumulated_reasoning() {
        let chunk1 = r#"{"id":"chatcmpl-123","object":"chat.completion.chunk","created":1234567890,"model":"gpt-4o","choices":[{"index":0,"delta":{"role":"assistant","reasoning":"think"},"finish_reason":null}]}"#;
        let chunk2 = r#"{"id":"chatcmpl-123","object":"chat.completion.chunk","created":1234567890,"model":"gpt-4o","choices":[{"index":0,"delta":{"role":"assistant","reasoning":" more"},"finish_reason":null}]}"#;
        let finish = r#"{"id":"chatcmpl-123","object":"chat.completion.chunk","created":1234567890,"model":"gpt-4o","choices":[{"index":0,"delta":{},"finish_reason":"stop"}]}"#;

        let mut state = ChatSseState::new();
        let _events1 = handle_chat_sse_event(&serde_json::from_str(chunk1).unwrap(), &mut state);
        let _events2 = handle_chat_sse_event(&serde_json::from_str(chunk2).unwrap(), &mut state);
        let events3 = handle_chat_sse_event(&serde_json::from_str(finish).unwrap(), &mut state);

        match &events3[0] {
            Ok(ResponseEvent::OutputItemDone { item, .. }) => match item {
                ResponseItem::Reasoning { content, .. } => {
                    assert!(matches!(
                        content.as_deref(),
                        Some([ReasoningItemContent::ReasoningText { text }]) if text == "think more"
                    ));
                }
                other => panic!("expected Reasoning, got {other:?}"),
            },
            other => panic!("expected OutputItemDone, got {other:?}"),
        }
    }

    #[test]
    fn empty_finish_reason_emits_no_completed() {
        // Some APIs (e.g., DeepSeek) return "" as finish_reason for "still streaming".
        let json = r#"{"id":"chatcmpl-123","object":"chat.completion.chunk","created":1234567890,"model":"gpt-4o","choices":[{"index":0,"delta":{"role":"assistant","content":"Hi"},"finish_reason":""}]}"#;
        let mut state = ChatSseState::new();
        let events = handle_chat_sse_event(&serde_json::from_str(json).unwrap(), &mut state);
        // Should emit OutputItemAdded + OutputTextDelta, but NOT Completed
        assert!(
            !events
                .iter()
                .any(|e| matches!(e, Ok(ResponseEvent::Completed { .. })))
        );
        assert!(matches!(&events[0], Ok(ResponseEvent::OutputItemAdded(_))));
        assert!(matches!(&events[1], Ok(ResponseEvent::OutputTextDelta(t)) if t == "Hi"));
    }

    #[test]
    fn finish_reason_tool_calls_emits_completed_with_end_turn_false() {
        let json = r#"{"id":"chatcmpl-123","object":"chat.completion.chunk","created":1234567890,"model":"gpt-4o","choices":[{"index":0,"delta":{},"finish_reason":"tool_calls"}],"usage":{"prompt_tokens":10,"completion_tokens":5,"total_tokens":15}}"#;
        let mut state = ChatSseState::new();
        let events = handle_chat_sse_event(&serde_json::from_str(json).unwrap(), &mut state);
        // OutputItemDone + Completed
        assert_eq!(events.len(), 2);
        assert!(matches!(
            &events[0],
            Ok(ResponseEvent::OutputItemDone { .. })
        ));
        match &events[1] {
            Ok(ResponseEvent::Completed { end_turn, .. }) => {
                assert_eq!(end_turn, &Some(false));
            }
            other => panic!("expected Completed, got {other:?}"),
        }
    }

    #[test]
    fn finish_reason_length_emits_completed_with_end_turn_false() {
        let json = r#"{"id":"chatcmpl-123","object":"chat.completion.chunk","created":1234567890,"model":"gpt-4o","choices":[{"index":0,"delta":{},"finish_reason":"length"}],"usage":{"prompt_tokens":10,"completion_tokens":5,"total_tokens":15}}"#;
        let mut state = ChatSseState::new();
        let events = handle_chat_sse_event(&serde_json::from_str(json).unwrap(), &mut state);
        assert_eq!(events.len(), 2);
        assert!(matches!(
            &events[0],
            Ok(ResponseEvent::OutputItemDone { .. })
        ));
        match &events[1] {
            Ok(ResponseEvent::Completed { end_turn, .. }) => {
                assert_eq!(end_turn, &Some(false));
            }
            other => panic!("expected Completed, got {other:?}"),
        }
    }

    #[test]
    fn finish_reason_content_filter_emits_completed_with_end_turn_none() {
        let json = r#"{"id":"chatcmpl-123","object":"chat.completion.chunk","created":1234567890,"model":"gpt-4o","choices":[{"index":0,"delta":{},"finish_reason":"content_filter"}],"usage":{"prompt_tokens":10,"completion_tokens":5,"total_tokens":15}}"#;
        let mut state = ChatSseState::new();
        let events = handle_chat_sse_event(&serde_json::from_str(json).unwrap(), &mut state);
        assert_eq!(events.len(), 2);
        assert!(matches!(
            &events[0],
            Ok(ResponseEvent::OutputItemDone { .. })
        ));
        match &events[1] {
            Ok(ResponseEvent::Completed { end_turn, .. }) => {
                assert_eq!(end_turn, &None);
            }
            other => panic!("expected Completed, got {other:?}"),
        }
    }

    #[test]
    fn test_map_finish_reason_to_end_turn() {
        assert_eq!(map_finish_reason_to_end_turn("stop"), Some(true));
        assert_eq!(map_finish_reason_to_end_turn("tool_calls"), Some(false));
        assert_eq!(map_finish_reason_to_end_turn("length"), Some(false));
        assert_eq!(map_finish_reason_to_end_turn("content_filter"), None);
        assert_eq!(map_finish_reason_to_end_turn("unknown"), None);
    }
}
