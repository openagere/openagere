use std::time::Duration;

use agere_app_server_protocol::JSONRPCResponse;
use agere_app_server_protocol::ModelProviderCapabilitiesReadParams;
use agere_app_server_protocol::ModelProviderCapabilitiesReadResponse;
use agere_app_server_protocol::RequestId;
use anyhow::Result;
use app_test_support::McpProcess;
use app_test_support::to_response;
use pretty_assertions::assert_eq;
use tempfile::TempDir;
use tokio::time::timeout;

const DEFAULT_TIMEOUT: Duration = Duration::from_secs(30);

#[tokio::test]
#[ignore]
async fn read_default_provider_capabilities() -> Result<()> {
    let agere_home = TempDir::new()?;
    let mut mcp = McpProcess::new(agere_home.path()).await?;
    timeout(DEFAULT_TIMEOUT, mcp.initialize()).await??;

    let request_id = mcp
        .send_model_provider_capabilities_read_request(ModelProviderCapabilitiesReadParams {})
        .await?;
    let response: JSONRPCResponse = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await??;
    let received: ModelProviderCapabilitiesReadResponse = to_response(response)?;

    let expected = ModelProviderCapabilitiesReadResponse {
        namespace_tools: true,
        image_generation: true,
        web_search: true,
    };
    assert_eq!(received, expected);
    Ok(())
}

#[tokio::test]
#[ignore]
async fn read_amazon_bedrock_provider_capabilities() -> Result<()> {
    let agere_home = TempDir::new()?;
    std::fs::write(
        agere_home.path().join("config.toml"),
        r#"model_provider = "amazon-bedrock"
"#,
    )?;
    let mut mcp = McpProcess::new(agere_home.path()).await?;
    timeout(DEFAULT_TIMEOUT, mcp.initialize()).await??;

    let request_id = mcp
        .send_model_provider_capabilities_read_request(ModelProviderCapabilitiesReadParams {})
        .await?;
    let response: JSONRPCResponse = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await??;
    let received: ModelProviderCapabilitiesReadResponse = to_response(response)?;

    let expected = ModelProviderCapabilitiesReadResponse {
        namespace_tools: false,
        image_generation: false,
        web_search: false,
    };
    assert_eq!(received, expected);
    Ok(())
}
