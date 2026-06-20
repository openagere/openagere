use agere_config::types::RateLimitRetryConfig;
use agere_config::types::RateLimitRetryToml;
use agere_model_provider_info::ModelProviderInfo;
use agere_model_provider_info::WireApi;
use agere_protocol::protocol::EventMsg;
use agere_protocol::protocol::Op;
use agere_protocol::user_input::UserInput;
use core_test_support::responses;
use core_test_support::skip_if_no_network;
use core_test_support::test_agere::TestAgere;
use core_test_support::test_agere::test_agere;
use core_test_support::wait_for_event;
use serde_json::json;
use wiremock::MockServer;
use wiremock::ResponseTemplate;

fn provider(base_url: String, stream_max_retries: u64) -> ModelProviderInfo {
    ModelProviderInfo {
        base_url: Some(base_url),
        env_key: Some("PATH".into()),
        env_key_instructions: None,
        experimental_bearer_token: None,
        auth: None,
        aws: None,
        wire_api: WireApi::Responses,
        query_params: None,
        http_headers: None,
        env_http_headers: None,
        request_max_retries: Some(0),
        stream_max_retries: Some(stream_max_retries),
        stream_idle_timeout_ms: Some(2_000),
        websocket_connect_timeout_ms: None,
        requires_provider_auth: false,
        supports_websockets: false,
    }
}

fn rate_limited_response() -> ResponseTemplate {
    ResponseTemplate::new(429)
        .insert_header("content-type", "application/json")
        .set_body_json(json!({
            "error": {
                "type": "rate_limit_exceeded",
                "message": "synthetic transient 429"
            }
        }))
}

fn successful_response() -> ResponseTemplate {
    responses::sse_response(responses::sse(vec![
        responses::ev_response_created("resp-ok"),
        responses::ev_completed("resp-ok"),
    ]))
}

fn rate_limited_stream_response() -> ResponseTemplate {
    responses::sse_response(responses::sse_failed(
        "resp-rate-limited",
        "rate_limit_exceeded",
        "synthetic streamed 429. Please try again in 1s.",
    ))
}

fn retryable_stream_failure_response() -> ResponseTemplate {
    responses::sse_response(responses::sse_failed(
        "resp-stream-failure",
        "server_error",
        "synthetic retryable stream failure",
    ))
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn slow_retry_starts_after_normal_retries_are_exhausted() -> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));
    let server = MockServer::start().await;
    let request_log = responses::mount_response_sequence(
        &server,
        vec![
            rate_limited_response(),
            rate_limited_response(),
            rate_limited_response(),
            successful_response(),
        ],
    )
    .await;
    let base_url = format!("{}/v1", server.uri());

    let TestAgere { agere, .. } = test_agere()
        .with_config(move |config| {
            config.model_provider = provider(base_url, 2);
            config.rate_limit_retry = RateLimitRetryConfig::from(RateLimitRetryToml {
                delays_secs: Some(vec![1]),
                cap_secs: Some(1),
                ..RateLimitRetryToml::default()
            });
        })
        .build(&server)
        .await?;

    agere
        .submit(Op::UserInput {
            environments: None,
            items: vec![UserInput::Text {
                text: "trigger transient 429".into(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
            responsesapi_client_metadata: None,
        })
        .await?;

    let waiting_event =
        wait_for_event(&agere, |ev| matches!(ev, EventMsg::RateLimitWaiting(_))).await;
    let EventMsg::RateLimitWaiting(waiting_event) = waiting_event else {
        unreachable!();
    };
    assert_eq!(waiting_event.attempt, 1);
    assert_eq!(waiting_event.wait_seconds, 1);
    assert_eq!(request_log.requests().len(), 3);

    wait_for_event(&agere, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;
    assert_eq!(request_log.requests().len(), 4);

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn slow_retry_handles_streamed_rate_limit_failures() -> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));
    let server = MockServer::start().await;
    let request_log = responses::mount_response_sequence(
        &server,
        vec![
            rate_limited_stream_response(),
            rate_limited_stream_response(),
            successful_response(),
        ],
    )
    .await;
    let base_url = format!("{}/v1", server.uri());

    let TestAgere { agere, .. } = test_agere()
        .with_config(move |config| {
            config.model_provider = provider(base_url, 1);
            config.rate_limit_retry = RateLimitRetryConfig::from(RateLimitRetryToml {
                delays_secs: Some(vec![1]),
                cap_secs: Some(1),
                ..RateLimitRetryToml::default()
            });
        })
        .build(&server)
        .await?;

    agere
        .submit(Op::UserInput {
            environments: None,
            items: vec![UserInput::Text {
                text: "trigger streamed transient 429".into(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
            responsesapi_client_metadata: None,
        })
        .await?;

    let waiting_event =
        wait_for_event(&agere, |ev| matches!(ev, EventMsg::RateLimitWaiting(_))).await;
    let EventMsg::RateLimitWaiting(waiting_event) = waiting_event else {
        unreachable!();
    };
    assert_eq!(waiting_event.attempt, 1);
    assert_eq!(waiting_event.wait_seconds, 1);
    assert_eq!(request_log.requests().len(), 2);

    wait_for_event(&agere, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;
    assert_eq!(request_log.requests().len(), 3);

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn slow_retry_restores_normal_retry_budget_after_waiting() -> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));
    let server = MockServer::start().await;
    let request_log = responses::mount_response_sequence(
        &server,
        vec![
            rate_limited_response(),
            rate_limited_response(),
            retryable_stream_failure_response(),
            successful_response(),
        ],
    )
    .await;
    let base_url = format!("{}/v1", server.uri());

    let TestAgere { agere, .. } = test_agere()
        .with_config(move |config| {
            config.model_provider = provider(base_url, 1);
            config.rate_limit_retry = RateLimitRetryConfig::from(RateLimitRetryToml {
                delays_secs: Some(vec![1]),
                cap_secs: Some(1),
                ..RateLimitRetryToml::default()
            });
        })
        .build(&server)
        .await?;

    agere
        .submit(Op::UserInput {
            environments: None,
            items: vec![UserInput::Text {
                text: "trigger transient 429 then stream failure".into(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
            responsesapi_client_metadata: None,
        })
        .await?;

    let waiting_event =
        wait_for_event(&agere, |ev| matches!(ev, EventMsg::RateLimitWaiting(_))).await;
    let EventMsg::RateLimitWaiting(waiting_event) = waiting_event else {
        unreachable!();
    };
    assert_eq!(waiting_event.attempt, 1);
    assert_eq!(waiting_event.wait_seconds, 1);
    assert_eq!(request_log.requests().len(), 2);

    wait_for_event(&agere, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;
    assert_eq!(request_log.requests().len(), 4);

    Ok(())
}
