use agere_app_server_protocol::AccessMode;
use agere_app_server_protocol::AppConfig;
use agere_app_server_protocol::AppToolApproval;
use agere_app_server_protocol::AppsConfig;
use agere_app_server_protocol::AskForApproval;
use agere_app_server_protocol::ConfigBatchWriteParams;
use agere_app_server_protocol::ConfigEdit;
use agere_app_server_protocol::ConfigLayerSource;
use agere_app_server_protocol::ConfigReadParams;
use agere_app_server_protocol::ConfigReadResponse;
use agere_app_server_protocol::ConfigValueWriteParams;
use agere_app_server_protocol::ConfigWriteResponse;
use agere_app_server_protocol::JSONRPCError;
use agere_app_server_protocol::JSONRPCResponse;
use agere_app_server_protocol::MergeStrategy;
use agere_app_server_protocol::RequestId;
use agere_app_server_protocol::ToolsV2;
use agere_app_server_protocol::WriteStatus;
use agere_core::config::set_project_trust_level;
use agere_protocol::config_types::TrustLevel;
use agere_protocol::config_types::WebSearchContextSize;
use agere_protocol::config_types::WebSearchLocation;
use agere_protocol::config_types::WebSearchToolConfig;
use agere_protocol::openai_models::ReasoningEffort;
use agere_utils_fs::AbsolutePathBuf;
use anyhow::Result;
use app_test_support::McpProcess;
use app_test_support::test_path_buf_with_windows;
use app_test_support::test_tmp_path_buf;
use app_test_support::to_response;
use pretty_assertions::assert_eq;
use serde_json::json;
use tempfile::TempDir;
use tokio::time::timeout;

// Bazel CI can spend tens of seconds starting app-server subprocesses or
// processing config RPCs under load.
const DEFAULT_READ_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(60);

fn write_config(agere_home: &TempDir, contents: &str) -> Result<()> {
    Ok(std::fs::write(
        agere_home.path().join("config.toml"),
        contents,
    )?)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore]
async fn config_read_returns_effective_and_layers() -> Result<()> {
    let agere_home = TempDir::new()?;
    write_config(
        &agere_home,
        r#"
model = "gpt-user"
access_mode = "workspace-write"
"#,
    )?;
    let agere_home_path = agere_home.path().canonicalize()?;
    let user_file = AbsolutePathBuf::try_from(agere_home_path.join("config.toml"))?;

    let mut mcp = McpProcess::new(agere_home.path()).await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

    let request_id = mcp
        .send_config_read_request(ConfigReadParams {
            include_layers: true,
            cwd: None,
        })
        .await?;
    let resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await??;
    let ConfigReadResponse {
        config,
        origins,
        layers,
    } = to_response(resp)?;

    assert_eq!(config.model.as_deref(), Some("gpt-user"));
    assert_eq!(
        origins.get("model").expect("origin").name,
        ConfigLayerSource::User {
            file: user_file.clone(),
        }
    );
    let layers = layers.expect("layers present");
    assert_layers_user_then_optional_system(&layers, user_file)?;

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore]
async fn config_read_includes_tools() -> Result<()> {
    let agere_home = TempDir::new()?;
    write_config(
        &agere_home,
        r#"
model = "gpt-user"

[tools.web_search]
context_size = "low"
allowed_domains = ["example.com"]

[tools]
view_image = false
"#,
    )?;
    let agere_home_path = agere_home.path().canonicalize()?;
    let user_file = AbsolutePathBuf::try_from(agere_home_path.join("config.toml"))?;

    let mut mcp = McpProcess::new(agere_home.path()).await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

    let request_id = mcp
        .send_config_read_request(ConfigReadParams {
            include_layers: true,
            cwd: None,
        })
        .await?;
    let resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await??;
    let ConfigReadResponse {
        config,
        origins,
        layers,
    } = to_response(resp)?;

    let tools = config.tools.expect("tools present");
    assert_eq!(
        tools,
        ToolsV2 {
            web_search: Some(WebSearchToolConfig {
                context_size: Some(WebSearchContextSize::Low),
                allowed_domains: Some(vec!["example.com".to_string()]),
                location: None,
            }),
            view_image: Some(false),
        }
    );
    assert_eq!(
        origins
            .get("tools.web_search.context_size")
            .expect("origin")
            .name,
        ConfigLayerSource::User {
            file: user_file.clone(),
        }
    );
    assert_eq!(
        origins
            .get("tools.web_search.allowed_domains.0")
            .expect("origin")
            .name,
        ConfigLayerSource::User {
            file: user_file.clone(),
        }
    );
    assert_eq!(
        origins.get("tools.view_image").expect("origin").name,
        ConfigLayerSource::User {
            file: user_file.clone(),
        }
    );

    let layers = layers.expect("layers present");
    assert_layers_user_then_optional_system(&layers, user_file)?;

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore]
async fn config_read_includes_nested_web_search_tool_config() -> Result<()> {
    let agere_home = TempDir::new()?;
    write_config(
        &agere_home,
        r#"
web_search = "live"

[tools.web_search]
context_size = "high"
allowed_domains = ["example.com"]
location = { country = "US", city = "New York", timezone = "America/New_York" }
"#,
    )?;

    let mut mcp = McpProcess::new(agere_home.path()).await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

    let request_id = mcp
        .send_config_read_request(ConfigReadParams {
            include_layers: false,
            cwd: None,
        })
        .await?;
    let resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await??;
    let ConfigReadResponse { config, .. } = to_response(resp)?;

    assert_eq!(
        config.tools.expect("tools present").web_search,
        Some(WebSearchToolConfig {
            context_size: Some(WebSearchContextSize::High),
            allowed_domains: Some(vec!["example.com".to_string()]),
            location: Some(WebSearchLocation {
                country: Some("US".to_string()),
                region: None,
                city: Some("New York".to_string()),
                timezone: Some("America/New_York".to_string()),
            }),
        }),
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore]
async fn config_read_ignores_bool_web_search_tool_config() -> Result<()> {
    let agere_home = TempDir::new()?;
    write_config(
        &agere_home,
        r#"
[tools]
web_search = true
"#,
    )?;

    let mut mcp = McpProcess::new(agere_home.path()).await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

    let request_id = mcp
        .send_config_read_request(ConfigReadParams {
            include_layers: false,
            cwd: None,
        })
        .await?;
    let resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await??;
    let ConfigReadResponse { config, .. } = to_response(resp)?;

    assert_eq!(config.tools.expect("tools present").web_search, None,);

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore]
async fn config_read_includes_apps() -> Result<()> {
    let agere_home = TempDir::new()?;
    write_config(
        &agere_home,
        r#"
[apps.app1]
enabled = false
destructive_enabled = false
default_tools_approval_mode = "prompt"
"#,
    )?;
    let agere_home_path = agere_home.path().canonicalize()?;
    let user_file = AbsolutePathBuf::try_from(agere_home_path.join("config.toml"))?;

    let mut mcp = McpProcess::new(agere_home.path()).await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

    let request_id = mcp
        .send_config_read_request(ConfigReadParams {
            include_layers: true,
            cwd: None,
        })
        .await?;
    let resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await??;
    let ConfigReadResponse {
        config,
        origins,
        layers,
    } = to_response(resp)?;

    assert_eq!(
        config.apps,
        Some(AppsConfig {
            default: None,
            apps: std::collections::HashMap::from([(
                "app1".to_string(),
                AppConfig {
                    enabled: false,
                    destructive_enabled: Some(false),
                    open_world_enabled: None,
                    default_tools_approval_mode: Some(AppToolApproval::Prompt),
                    default_tools_enabled: None,
                    tools: None,
                },
            )]),
        })
    );
    assert_eq!(
        origins.get("apps.app1.enabled").expect("origin").name,
        ConfigLayerSource::User {
            file: user_file.clone(),
        }
    );
    assert_eq!(
        origins
            .get("apps.app1.destructive_enabled")
            .expect("origin")
            .name,
        ConfigLayerSource::User {
            file: user_file.clone(),
        }
    );
    assert_eq!(
        origins
            .get("apps.app1.default_tools_approval_mode")
            .expect("origin")
            .name,
        ConfigLayerSource::User {
            file: user_file.clone(),
        }
    );

    let layers = layers.expect("layers present");
    assert_layers_user_then_optional_system(&layers, user_file)?;

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore]
async fn config_read_includes_project_layers_for_cwd() -> Result<()> {
    let agere_home = TempDir::new()?;
    write_config(&agere_home, r#"model = "gpt-user""#)?;

    let workspace = TempDir::new()?;
    let project_config_dir = workspace.path().join(".openagere");
    std::fs::create_dir_all(&project_config_dir)?;
    std::fs::write(
        project_config_dir.join("config.toml"),
        r#"
model_reasoning_effort = "high"
"#,
    )?;
    set_project_trust_level(agere_home.path(), workspace.path(), TrustLevel::Trusted)?;
    let project_config = AbsolutePathBuf::try_from(project_config_dir)?;

    let mut mcp = McpProcess::new(agere_home.path()).await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

    let request_id = mcp
        .send_config_read_request(ConfigReadParams {
            include_layers: true,
            cwd: Some(workspace.path().to_string_lossy().into_owned()),
        })
        .await?;
    let resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await??;
    let ConfigReadResponse {
        config, origins, ..
    } = to_response(resp)?;

    assert_eq!(config.model_reasoning_effort, Some(ReasoningEffort::High));
    assert_eq!(
        origins.get("model_reasoning_effort").expect("origin").name,
        ConfigLayerSource::Project {
            dot_agere_folder: project_config
        }
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore]
async fn config_read_includes_system_layer_and_overrides() -> Result<()> {
    let agere_home = TempDir::new()?;
    let user_dir = test_path_buf_with_windows("/user", Some(r"C:\Users\user"));
    let system_dir = test_path_buf_with_windows("/system", Some(r"C:\System"));
    write_config(
        &agere_home,
        &format!(
            r#"
model = "gpt-user"
approval_policy = "on-request"
access_mode = "workspace-write"

[access_workspace_write]
writable_roots = [{}]
network_access = true
"#,
            serde_json::json!(user_dir)
        ),
    )?;
    let agere_home_path = agere_home.path().canonicalize()?;
    let user_file = AbsolutePathBuf::try_from(agere_home_path.join("config.toml"))?;

    let managed_path = agere_home.path().join("managed_config.toml");
    let managed_file = AbsolutePathBuf::try_from(managed_path.clone())?;
    std::fs::write(
        &managed_path,
        format!(
            r#"
model = "gpt-system"
approval_policy = "never"

[access_workspace_write]
writable_roots = [{}]
"#,
            serde_json::json!(system_dir.clone())
        ),
    )?;

    let managed_path_str = managed_path.display().to_string();

    let mut mcp = McpProcess::new_with_env(
        agere_home.path(),
        &[(
            "AGERE_APP_SERVER_MANAGED_CONFIG_PATH",
            Some(&managed_path_str),
        )],
    )
    .await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

    let request_id = mcp
        .send_config_read_request(ConfigReadParams {
            include_layers: true,
            cwd: None,
        })
        .await?;
    let resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await??;
    let ConfigReadResponse {
        config,
        origins,
        layers,
    } = to_response(resp)?;

    assert_eq!(config.model.as_deref(), Some("gpt-system"));
    assert_eq!(
        origins.get("model").expect("origin").name,
        ConfigLayerSource::LegacyManagedConfigTomlFromFile {
            file: managed_file.clone(),
        }
    );

    assert_eq!(config.approval_policy, Some(AskForApproval::Never));
    assert_eq!(
        origins.get("approval_policy").expect("origin").name,
        ConfigLayerSource::LegacyManagedConfigTomlFromFile {
            file: managed_file.clone(),
        }
    );

    assert_eq!(config.access_mode, Some(AccessMode::WorkspaceWrite));
    assert_eq!(
        origins.get("access_mode").expect("origin").name,
        ConfigLayerSource::User {
            file: user_file.clone(),
        }
    );

    let access_workspace_write = config
        .access_workspace_write
        .as_ref()
        .expect("access workspace write");
    assert_eq!(access_workspace_write.writable_roots, vec![system_dir]);
    assert_eq!(
        origins
            .get("access_workspace_write.writable_roots.0")
            .expect("origin")
            .name,
        ConfigLayerSource::LegacyManagedConfigTomlFromFile {
            file: managed_file.clone(),
        }
    );

    assert!(access_workspace_write.network_access);
    assert_eq!(
        origins
            .get("access_workspace_write.network_access")
            .expect("origin")
            .name,
        ConfigLayerSource::User {
            file: user_file.clone(),
        }
    );

    let layers = layers.expect("layers present");
    assert_layers_managed_user_then_optional_system(&layers, managed_file, user_file)?;

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore]
async fn config_value_write_replaces_value() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let agere_home = temp_dir.path().canonicalize()?;
    write_config(
        &temp_dir,
        r#"
model = "gpt-old"
"#,
    )?;

    let mut mcp = McpProcess::new(&agere_home).await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

    let read_id = mcp
        .send_config_read_request(ConfigReadParams {
            include_layers: false,
            cwd: None,
        })
        .await?;
    let read_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(read_id)),
    )
    .await??;
    let read: ConfigReadResponse = to_response(read_resp)?;
    let expected_version = read.origins.get("model").map(|m| m.version.clone());

    let write_id = mcp
        .send_config_value_write_request(ConfigValueWriteParams {
            file_path: None,
            key_path: "model".to_string(),
            value: json!("gpt-new"),
            merge_strategy: MergeStrategy::Replace,
            expected_version,
        })
        .await?;
    let write_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(write_id)),
    )
    .await??;
    let write: ConfigWriteResponse = to_response(write_resp)?;
    let expected_file_path = AbsolutePathBuf::resolve_path_against_base("config.toml", agere_home);

    assert_eq!(write.status, WriteStatus::Ok);
    assert_eq!(write.file_path, expected_file_path);
    assert!(write.overridden_metadata.is_none());

    let verify_id = mcp
        .send_config_read_request(ConfigReadParams {
            include_layers: false,
            cwd: None,
        })
        .await?;
    let verify_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(verify_id)),
    )
    .await??;
    let verify: ConfigReadResponse = to_response(verify_resp)?;
    assert_eq!(verify.config.model.as_deref(), Some("gpt-new"));

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore]
async fn config_read_after_pipelined_write_sees_written_value() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let agere_home = temp_dir.path().canonicalize()?;
    write_config(
        &temp_dir,
        r#"
model = "gpt-old"
"#,
    )?;

    let mut mcp = McpProcess::new(&agere_home).await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

    let write_id = mcp
        .send_config_value_write_request(ConfigValueWriteParams {
            file_path: None,
            key_path: "model".to_string(),
            value: json!("gpt-new"),
            merge_strategy: MergeStrategy::Replace,
            expected_version: None,
        })
        .await?;
    let read_id = mcp
        .send_config_read_request(ConfigReadParams {
            include_layers: false,
            cwd: None,
        })
        .await?;

    let write_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(write_id)),
    )
    .await??;
    let write: ConfigWriteResponse = to_response(write_resp)?;
    assert_eq!(write.status, WriteStatus::Ok);

    let read_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(read_id)),
    )
    .await??;
    let read: ConfigReadResponse = to_response(read_resp)?;
    assert_eq!(read.config.model.as_deref(), Some("gpt-new"));

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore]
async fn config_value_write_rejects_version_conflict() -> Result<()> {
    let agere_home = TempDir::new()?;
    write_config(
        &agere_home,
        r#"
model = "gpt-old"
"#,
    )?;

    let mut mcp = McpProcess::new(agere_home.path()).await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

    let write_id = mcp
        .send_config_value_write_request(ConfigValueWriteParams {
            file_path: Some(agere_home.path().join("config.toml").display().to_string()),
            key_path: "model".to_string(),
            value: json!("gpt-new"),
            merge_strategy: MergeStrategy::Replace,
            expected_version: Some("sha256:stale".to_string()),
        })
        .await?;

    let err: JSONRPCError = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_error_message(RequestId::Integer(write_id)),
    )
    .await??;
    let code = err
        .error
        .data
        .as_ref()
        .and_then(|d| d.get("config_write_error_code"))
        .and_then(|v| v.as_str());
    assert_eq!(code, Some("configVersionConflict"));

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore]
async fn config_batch_write_applies_multiple_edits() -> Result<()> {
    let tmp_dir = TempDir::new()?;
    let agere_home = tmp_dir.path().canonicalize()?;
    write_config(&tmp_dir, "")?;

    let mut mcp = McpProcess::new(&agere_home).await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

    let writable_root = test_tmp_path_buf();
    let batch_id = mcp
        .send_config_batch_write_request(ConfigBatchWriteParams {
            file_path: Some(agere_home.join("config.toml").display().to_string()),
            edits: vec![
                ConfigEdit {
                    key_path: "access_mode".to_string(),
                    value: json!("workspace-write"),
                    merge_strategy: MergeStrategy::Replace,
                },
                ConfigEdit {
                    key_path: "access_workspace_write".to_string(),
                    value: json!({
                        "writable_roots": [writable_root.clone()],
                        "network_access": false
                    }),
                    merge_strategy: MergeStrategy::Replace,
                },
            ],
            expected_version: None,
            reload_user_config: false,
        })
        .await?;
    let batch_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(batch_id)),
    )
    .await??;
    let batch_write: ConfigWriteResponse = to_response(batch_resp)?;
    assert_eq!(batch_write.status, WriteStatus::Ok);
    let expected_file_path = AbsolutePathBuf::resolve_path_against_base("config.toml", agere_home);
    assert_eq!(batch_write.file_path, expected_file_path);

    let read_id = mcp
        .send_config_read_request(ConfigReadParams {
            include_layers: false,
            cwd: None,
        })
        .await?;
    let read_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(read_id)),
    )
    .await??;
    let read: ConfigReadResponse = to_response(read_resp)?;
    assert_eq!(read.config.access_mode, Some(AccessMode::WorkspaceWrite));
    let access_workspace_write = read
        .config
        .access_workspace_write
        .as_ref()
        .expect("access workspace write");
    assert_eq!(access_workspace_write.writable_roots, vec![writable_root]);
    assert!(!access_workspace_write.network_access);

    Ok(())
}

fn assert_layers_user_then_optional_system(
    layers: &[agere_app_server_protocol::ConfigLayer],
    user_file: AbsolutePathBuf,
) -> Result<()> {
    let mut first_index = 0;
    if matches!(
        layers.first().map(|layer| &layer.name),
        Some(ConfigLayerSource::LegacyManagedConfigTomlFromMdm)
    ) {
        first_index = 1;
    }
    assert_eq!(layers.len(), first_index + 2);
    assert_eq!(
        layers[first_index].name,
        ConfigLayerSource::User { file: user_file }
    );
    assert!(matches!(
        layers[first_index + 1].name,
        ConfigLayerSource::System { .. }
    ));
    Ok(())
}

fn assert_layers_managed_user_then_optional_system(
    layers: &[agere_app_server_protocol::ConfigLayer],
    managed_file: AbsolutePathBuf,
    user_file: AbsolutePathBuf,
) -> Result<()> {
    let mut first_index = 0;
    if matches!(
        layers.first().map(|layer| &layer.name),
        Some(ConfigLayerSource::LegacyManagedConfigTomlFromMdm)
    ) {
        first_index = 1;
    }
    assert_eq!(layers.len(), first_index + 3);
    assert_eq!(
        layers[first_index].name,
        ConfigLayerSource::LegacyManagedConfigTomlFromFile { file: managed_file }
    );
    assert_eq!(
        layers[first_index + 1].name,
        ConfigLayerSource::User { file: user_file }
    );
    assert!(matches!(
        layers[first_index + 2].name,
        ConfigLayerSource::System { .. }
    ));
    Ok(())
}
