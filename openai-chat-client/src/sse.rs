use crate::translate::response::ChatSseState;
use crate::translate::response::handle_chat_sse_event;
use crate::types::ChatSseEvent;
use agere_api::ApiError;
use agere_api::ResponseEvent;
use agere_client::ByteStream;
use eventsource_stream::Eventsource;
use futures::StreamExt;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::time::timeout;
use tracing::debug;
use tracing::info;
use tracing::trace;

/// Process Chat Completions SSE byte stream, emitting ResponseEvents to the channel.
pub(crate) async fn process_chat_sse(
    stream: ByteStream,
    tx_event: mpsc::Sender<Result<ResponseEvent, ApiError>>,
    idle_timeout: Duration,
) {
    let mut sse_stream = stream.eventsource();
    let mut state = ChatSseState::new();
    let mut line_count: u32 = 0;

    info!("Chat SSE: stream started");

    loop {
        let response = timeout(idle_timeout, sse_stream.next()).await;

        let sse = match response {
            Ok(Some(Ok(sse))) => sse,
            Ok(Some(Err(e))) => {
                info!("Chat SSE error: {e:#}");
                if flush_pending_completion_before_terminal_error(
                    &mut state,
                    &tx_event,
                    "stream error",
                )
                .await
                {
                    return;
                }
                let _ = tx_event.send(Err(ApiError::Stream(e.to_string()))).await;
                return;
            }
            Ok(None) => {
                let results = handle_chat_sse_event(&ChatSseEvent::Done, &mut state);
                if !results.is_empty() {
                    for result in results {
                        if tx_event.send(result).await.is_err() {
                            info!("Chat SSE EOF: receiver dropped");
                            return;
                        }
                    }
                    return;
                }
                if state.completed {
                    info!("Chat SSE: stream closed after completion (lines={line_count})");
                    return;
                }
                info!(
                    "Chat SSE: stream closed before completion (lines={})",
                    line_count
                );
                let _ = tx_event
                    .send(Err(ApiError::Stream(
                        "stream closed before completion".into(),
                    )))
                    .await;
                return;
            }
            Err(_) => {
                info!("Chat SSE: idle timeout waiting for SSE");
                if flush_pending_completion_before_terminal_error(
                    &mut state,
                    &tx_event,
                    "idle timeout",
                )
                .await
                {
                    return;
                }
                let _ = tx_event
                    .send(Err(ApiError::Stream("idle timeout waiting for SSE".into())))
                    .await;
                return;
            }
        };

        line_count += 1;

        trace!("Chat SSE raw event line {}: {}", line_count, &sse.data);

        // Skip [DONE] marker
        if sse.data.trim() == "[DONE]" {
            info!("Chat SSE: [DONE] marker at line {}", line_count);
            let results = handle_chat_sse_event(&ChatSseEvent::Done, &mut state);
            if results.is_empty() {
                if state.completed {
                    return;
                }
                let _ = tx_event
                    .send(Err(ApiError::Stream(
                        "stream closed before completion".into(),
                    )))
                    .await;
                return;
            }
            for result in results {
                if tx_event.send(result).await.is_err() {
                    info!("Chat SSE line {}: receiver dropped", line_count);
                    return;
                }
            }
            return;
        }

        let event = match serde_json::from_str(&sse.data) {
            Ok(event) => event,
            Err(e) => {
                debug!("Failed to parse Chat SSE: {e}, data: {}", &sse.data);
                continue;
            }
        };

        info!("Chat SSE line {}: parsed event", line_count);

        for result in handle_chat_sse_event(&event, &mut state) {
            match &result {
                Ok(ResponseEvent::OutputItemAdded(_)) => {
                    info!("Chat SSE line {}: OutputItemAdded", line_count);
                }
                Ok(ResponseEvent::OutputTextDelta(text)) => {
                    info!(
                        "Chat SSE line {}: OutputTextDelta({:?})",
                        line_count,
                        text.chars().take(40).collect::<String>()
                    );
                }
                Ok(ResponseEvent::Completed {
                    end_turn,
                    token_usage,
                    ..
                }) => {
                    info!(
                        "Chat SSE line {}: Completed(end_turn={:?}, tokens={:?})",
                        line_count, end_turn, token_usage
                    );
                }
                Ok(_) => {
                    info!("Chat SSE line {}: other event", line_count);
                }
                Err(e) => {
                    info!("Chat SSE line {}: error: {:?}", line_count, e);
                }
            }
            if tx_event.send(result).await.is_err() {
                info!("Chat SSE line {}: receiver dropped", line_count);
                return;
            }
        }
    }
}

async fn flush_pending_completion_before_terminal_error(
    state: &mut ChatSseState,
    tx_event: &mpsc::Sender<Result<ResponseEvent, ApiError>>,
    reason: &str,
) -> bool {
    let results = handle_chat_sse_event(&ChatSseEvent::Done, state);
    if results.is_empty() {
        return false;
    }
    for result in results {
        if tx_event.send(result).await.is_err() {
            info!("Chat SSE {reason}: receiver dropped");
            return true;
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use agere_api::ResponseEvent;
    use bytes::Bytes;
    use futures::stream;
    use pretty_assertions::assert_eq;

    #[tokio::test]
    async fn full_text_lifecycle() {
        let body = concat!(
            "data: {\"id\":\"chatcmpl-123\",\"object\":\"chat.completion.chunk\",\"created\":1234567890,\"model\":\"gpt-4o\",\"choices\":[{\"index\":0,\"delta\":{\"role\":\"assistant\"},\"finish_reason\":null}]}\n\n",
            "data: {\"id\":\"chatcmpl-123\",\"object\":\"chat.completion.chunk\",\"created\":1234567890,\"model\":\"gpt-4o\",\"choices\":[{\"index\":0,\"delta\":{\"content\":\"Hello\"},\"finish_reason\":null}]}\n\n",
            "data: {\"id\":\"chatcmpl-123\",\"object\":\"chat.completion.chunk\",\"created\":1234567890,\"model\":\"gpt-4o\",\"choices\":[{\"index\":0,\"delta\":{},\"finish_reason\":\"stop\"}],\"usage\":{\"prompt_tokens\":10,\"completion_tokens\":3,\"total_tokens\":13}}\n\n",
        );

        let events = collect_sse(body).await;
        // OutputItemAdded(Message), OutputTextDelta(Hello), OutputItemDone(Message), Completed
        assert_eq!(events.len(), 4);
        assert!(matches!(&events[0], Ok(ResponseEvent::OutputItemAdded(_))));
        assert!(matches!(&events[1], Ok(ResponseEvent::OutputTextDelta(t)) if t == "Hello"));
        assert!(matches!(&events[2], Ok(ResponseEvent::OutputItemDone {
            item: agere_protocol::models::ResponseItem::Message { content, .. }, ..
        }) if matches!(content.first(), Some(agere_protocol::models::ContentItem::OutputText { text }) if text == "Hello")));
        assert!(matches!(
            &events[3],
            Ok(ResponseEvent::Completed {
                end_turn: Some(true),
                ..
            })
        ));
    }

    #[tokio::test]
    async fn usage_only_chunk_after_finish_completes_with_usage() {
        let body = concat!(
            "data: {\"id\":\"chatcmpl-123\",\"object\":\"chat.completion.chunk\",\"created\":1234567890,\"model\":\"gpt-4o\",\"choices\":[{\"index\":0,\"delta\":{\"role\":\"assistant\"},\"finish_reason\":null}]}\n\n",
            "data: {\"id\":\"chatcmpl-123\",\"object\":\"chat.completion.chunk\",\"created\":1234567890,\"model\":\"gpt-4o\",\"choices\":[{\"index\":0,\"delta\":{\"content\":\"Hello\"},\"finish_reason\":null}]}\n\n",
            "data: {\"id\":\"chatcmpl-123\",\"object\":\"chat.completion.chunk\",\"created\":1234567890,\"model\":\"gpt-4o\",\"choices\":[{\"index\":0,\"delta\":{},\"finish_reason\":\"stop\"}]}\n\n",
            "data: {\"id\":\"chatcmpl-123\",\"object\":\"chat.completion.chunk\",\"created\":1234567890,\"model\":\"gpt-4o\",\"choices\":[],\"usage\":{\"prompt_tokens\":10,\"completion_tokens\":3,\"total_tokens\":13}}\n\n",
        );

        let events = collect_sse(body).await;
        assert!(matches!(
            events.last(),
            Some(Ok(ResponseEvent::Completed {
                end_turn: Some(true),
                token_usage: Some(usage),
                ..
            })) if usage.input_tokens == 10
                && usage.output_tokens == 3
                && usage.total_tokens == 13
        ));
    }

    #[tokio::test]
    async fn done_after_finish_chunk_with_usage_is_clean_eof() {
        let body = concat!(
            "data: {\"id\":\"chatcmpl-123\",\"object\":\"chat.completion.chunk\",\"created\":1234567890,\"model\":\"gpt-4o\",\"choices\":[{\"index\":0,\"delta\":{\"role\":\"assistant\"},\"finish_reason\":null}]}\n\n",
            "data: {\"id\":\"chatcmpl-123\",\"object\":\"chat.completion.chunk\",\"created\":1234567890,\"model\":\"gpt-4o\",\"choices\":[{\"index\":0,\"delta\":{\"content\":\"Hello\"},\"finish_reason\":null}]}\n\n",
            "data: {\"id\":\"chatcmpl-123\",\"object\":\"chat.completion.chunk\",\"created\":1234567890,\"model\":\"gpt-4o\",\"choices\":[{\"index\":0,\"delta\":{},\"finish_reason\":\"stop\"}],\"usage\":{\"prompt_tokens\":10,\"completion_tokens\":3,\"total_tokens\":13}}\n\n",
            "data: [DONE]\n\n",
        );

        let events = collect_all_sse(body).await;
        assert!(matches!(
            events.last(),
            Some(Ok(ResponseEvent::Completed {
                end_turn: Some(true),
                ..
            }))
        ));
        assert!(
            events.iter().all(Result::is_ok),
            "expected no stream error after completion, got {events:?}"
        );
    }

    #[tokio::test]
    async fn eof_after_finish_chunk_without_usage_flushes_completion() {
        let body = concat!(
            "data: {\"id\":\"chatcmpl-123\",\"object\":\"chat.completion.chunk\",\"created\":1234567890,\"model\":\"gpt-4o\",\"choices\":[{\"index\":0,\"delta\":{\"role\":\"assistant\"},\"finish_reason\":null}]}\n\n",
            "data: {\"id\":\"chatcmpl-123\",\"object\":\"chat.completion.chunk\",\"created\":1234567890,\"model\":\"gpt-4o\",\"choices\":[{\"index\":0,\"delta\":{\"content\":\"Hello\"},\"finish_reason\":null}]}\n\n",
            "data: {\"id\":\"chatcmpl-123\",\"object\":\"chat.completion.chunk\",\"created\":1234567890,\"model\":\"gpt-4o\",\"choices\":[{\"index\":0,\"delta\":{},\"finish_reason\":\"stop\"}]}\n\n",
        );

        let events = collect_all_sse(body).await;
        assert!(matches!(
            events.last(),
            Some(Ok(ResponseEvent::Completed {
                end_turn: Some(true),
                token_usage: None,
                ..
            }))
        ));
        assert!(
            events.iter().all(Result::is_ok),
            "expected clean EOF after pending completion, got {events:?}"
        );
    }

    #[tokio::test]
    async fn stream_error_after_finish_chunk_without_usage_flushes_completion() {
        use agere_client::TransportError;

        let body = concat!(
            "data: {\"id\":\"chatcmpl-123\",\"object\":\"chat.completion.chunk\",\"created\":1234567890,\"model\":\"gpt-4o\",\"choices\":[{\"index\":0,\"delta\":{\"role\":\"assistant\"},\"finish_reason\":null}]}\n\n",
            "data: {\"id\":\"chatcmpl-123\",\"object\":\"chat.completion.chunk\",\"created\":1234567890,\"model\":\"gpt-4o\",\"choices\":[{\"index\":0,\"delta\":{\"content\":\"Hello\"},\"finish_reason\":null}]}\n\n",
            "data: {\"id\":\"chatcmpl-123\",\"object\":\"chat.completion.chunk\",\"created\":1234567890,\"model\":\"gpt-4o\",\"choices\":[{\"index\":0,\"delta\":{},\"finish_reason\":\"stop\"}]}\n\n",
        );
        let byte_stream: agere_client::ByteStream = Box::pin(stream::iter(vec![
            Ok(Bytes::from(body.to_string())),
            Err(TransportError::Network(
                "synthetic stream error".to_string(),
            )),
        ]));

        let events = collect_all_byte_stream(byte_stream, Duration::from_secs(5)).await;
        assert!(matches!(
            events.last(),
            Some(Ok(ResponseEvent::Completed {
                end_turn: Some(true),
                token_usage: None,
                ..
            }))
        ));
        assert!(
            events.iter().all(Result::is_ok),
            "expected pending completion to win over stream error, got {events:?}"
        );
    }

    #[tokio::test]
    async fn idle_timeout_after_finish_chunk_without_usage_flushes_completion() {
        let body = concat!(
            "data: {\"id\":\"chatcmpl-123\",\"object\":\"chat.completion.chunk\",\"created\":1234567890,\"model\":\"gpt-4o\",\"choices\":[{\"index\":0,\"delta\":{\"role\":\"assistant\"},\"finish_reason\":null}]}\n\n",
            "data: {\"id\":\"chatcmpl-123\",\"object\":\"chat.completion.chunk\",\"created\":1234567890,\"model\":\"gpt-4o\",\"choices\":[{\"index\":0,\"delta\":{\"content\":\"Hello\"},\"finish_reason\":null}]}\n\n",
            "data: {\"id\":\"chatcmpl-123\",\"object\":\"chat.completion.chunk\",\"created\":1234567890,\"model\":\"gpt-4o\",\"choices\":[{\"index\":0,\"delta\":{},\"finish_reason\":\"stop\"}]}\n\n",
        );
        let byte_stream: agere_client::ByteStream = Box::pin(
            stream::iter(vec![Ok(Bytes::from(body.to_string()))]).chain(stream::pending()),
        );

        let events = collect_all_byte_stream(byte_stream, Duration::from_millis(10)).await;
        assert!(matches!(
            events.last(),
            Some(Ok(ResponseEvent::Completed {
                end_turn: Some(true),
                token_usage: None,
                ..
            }))
        ));
        assert!(
            events.iter().all(Result::is_ok),
            "expected pending completion to win over idle timeout, got {events:?}"
        );
    }

    #[tokio::test]
    async fn done_before_finish_reason_emits_stream_closed_error() {
        let body = concat!(
            "data: {\"id\":\"chatcmpl-123\",\"object\":\"chat.completion.chunk\",\"created\":1234567890,\"model\":\"gpt-4o\",\"choices\":[{\"index\":0,\"delta\":{\"role\":\"assistant\"},\"finish_reason\":null}]}\n\n",
            "data: {\"id\":\"chatcmpl-123\",\"object\":\"chat.completion.chunk\",\"created\":1234567890,\"model\":\"gpt-4o\",\"choices\":[{\"index\":0,\"delta\":{\"content\":\"Hello\"},\"finish_reason\":null}]}\n\n",
            "data: [DONE]\n\n",
        );

        let events = collect_sse(body).await;
        assert!(matches!(&events[..], [
            Ok(ResponseEvent::OutputItemAdded(_)),
            Ok(ResponseEvent::OutputTextDelta(text)),
            Err(ApiError::Stream(message)),
        ] if text == "Hello" && message == "stream closed before completion"));
    }

    #[tokio::test]
    async fn tool_call_lifecycle() {
        let body = concat!(
            "data: {\"id\":\"chatcmpl-123\",\"object\":\"chat.completion.chunk\",\"created\":1234567890,\"model\":\"gpt-4o\",\"choices\":[{\"index\":0,\"delta\":{\"role\":\"assistant\",\"tool_calls\":[{\"index\":0,\"id\":\"call_abc\",\"type\":\"function\",\"function\":{\"name\":\"get_weather\"}}]},\"finish_reason\":null}]}\n\n",
            "data: {\"id\":\"chatcmpl-123\",\"object\":\"chat.completion.chunk\",\"created\":1234567890,\"model\":\"gpt-4o\",\"choices\":[{\"index\":0,\"delta\":{\"tool_calls\":[{\"index\":0,\"function\":{\"arguments\":\"{\\\"city\\\"\"}}]},\"finish_reason\":null}]}\n\n",
            "data: {\"id\":\"chatcmpl-123\",\"object\":\"chat.completion.chunk\",\"created\":1234567890,\"model\":\"gpt-4o\",\"choices\":[{\"index\":0,\"delta\":{\"tool_calls\":[{\"index\":0,\"function\":{\"arguments\":\":\\\"SF\\\"}\"}}]},\"finish_reason\":null}]}\n\n",
            "data: {\"id\":\"chatcmpl-123\",\"object\":\"chat.completion.chunk\",\"created\":1234567890,\"model\":\"gpt-4o\",\"choices\":[{\"index\":0,\"delta\":{},\"finish_reason\":\"tool_calls\"}]}\n\n",
            "data: [DONE]\n\n",
        );

        let events = collect_sse(body).await;
        // OutputItemAdded(FunctionCall), 2x ToolCallInputDelta,
        // OutputItemDone(FunctionCall), OutputItemDone(Message), Completed(end_turn=false)
        assert_eq!(events.len(), 6);
        assert!(matches!(&events[0], Ok(ResponseEvent::OutputItemAdded(_))));
        assert!(matches!(
            &events[1],
            Ok(ResponseEvent::ToolCallInputDelta { .. })
        ));
        assert!(matches!(
            &events[2],
            Ok(ResponseEvent::ToolCallInputDelta { .. })
        ));
        // events[3] = OutputItemDone(FunctionCall)
        assert!(matches!(
            &events[3],
            Ok(ResponseEvent::OutputItemDone {
                item: agere_protocol::models::ResponseItem::FunctionCall { .. },
                ..
            })
        ));
        // events[4] = OutputItemDone(Message)
        assert!(matches!(
            &events[4],
            Ok(ResponseEvent::OutputItemDone {
                item: agere_protocol::models::ResponseItem::Message { .. },
                ..
            })
        ));
        assert!(matches!(
            &events[5],
            Ok(ResponseEvent::Completed {
                end_turn: Some(false),
                ..
            })
        ));
    }

    async fn collect_sse(body: &str) -> Vec<Result<ResponseEvent, ApiError>> {
        use agere_client::TransportError;
        let chunks: Vec<Result<Bytes, TransportError>> = vec![Ok(Bytes::from(body.to_string()))];
        let byte_stream: agere_client::ByteStream = Box::pin(stream::iter(chunks));

        let (tx, mut rx) = tokio::sync::mpsc::channel::<Result<ResponseEvent, ApiError>>(16);
        tokio::spawn(process_chat_sse(byte_stream, tx, Duration::from_secs(5)));

        let mut events = Vec::new();
        while let Some(ev) = rx.recv().await {
            let is_terminal = matches!(ev, Ok(ResponseEvent::Completed { .. }) | Err(_));
            events.push(ev);
            if is_terminal {
                break;
            }
        }
        events
    }

    async fn collect_all_sse(body: &str) -> Vec<Result<ResponseEvent, ApiError>> {
        use agere_client::TransportError;
        let chunks: Vec<Result<Bytes, TransportError>> = vec![Ok(Bytes::from(body.to_string()))];
        let byte_stream: agere_client::ByteStream = Box::pin(stream::iter(chunks));

        collect_all_byte_stream(byte_stream, Duration::from_secs(5)).await
    }

    async fn collect_all_byte_stream(
        byte_stream: agere_client::ByteStream,
        idle_timeout: Duration,
    ) -> Vec<Result<ResponseEvent, ApiError>> {
        let (tx, mut rx) = tokio::sync::mpsc::channel::<Result<ResponseEvent, ApiError>>(16);
        tokio::spawn(process_chat_sse(byte_stream, tx, idle_timeout));

        let mut events = Vec::new();
        while let Some(ev) = rx.recv().await {
            events.push(ev);
        }
        events
    }
}
