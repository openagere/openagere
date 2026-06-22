//! Regression coverage for app-server thread operations backed by a non-local
//! `ThreadStore`.
//!
//! The app-server startup path should honor `experimental_thread_store`
//! by routing all thread persistence through the configured store. This suite uses
//! the thread-store crate's test-only in-memory store, which exercises the same
//! config-driven selection path as a remote store without requiring the real gRPC
//! service.
//!
//! The important failure mode is accidentally materializing local persistence
//! while a non-local store is configured. After `thread/start` and a simple turn,
//! the temporary `agere_home` must not contain rollout session files or sqlite
//! state files. This does not observe read-only probes that leave no artifact; it
//! is a stop-gap that prevents additional local persistence writes from slipping
//! in unnoticed.

use std::collections::BTreeSet;
use std::path::Path;
use std::sync::Arc;

use agere_app_server::in_process;
use agere_app_server::in_process::InProcessServerEvent;
use agere_app_server::in_process::InProcessStartArgs;
use agere_app_server_protocol::ClientInfo;
use agere_app_server_protocol::ClientRequest;
use agere_app_server_protocol::InitializeParams;
use agere_app_server_protocol::RequestId;
use agere_app_server_protocol::ServerNotification;
use agere_app_server_protocol::ThreadStartParams;
use agere_app_server_protocol::ThreadStartResponse;
use agere_app_server_protocol::TurnStartParams;
use agere_app_server_protocol::UserInput as V2UserInput;
use agere_arg0::Arg0DispatchPaths;
use agere_config::LoaderOverrides;
use agere_config::NoopThreadConfigLoader;
use agere_core::config::ConfigBuilder;
use agere_exec_server::EnvironmentManager;
use agere_feedback::AgereFeedback;
use agere_protocol::protocol::SessionSource;
use agere_thread_store::InMemoryThreadStore;
use anyhow::Result;
use app_test_support::create_mock_responses_server_repeating_assistant;
use pretty_assertions::assert_eq;
use tempfile::TempDir;
use tokio::time::timeout;
use uuid::Uuid;

const DEFAULT_READ_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);

#[tokio::test]
#[ignore]
async fn thread_start_with_non_local_thread_store_does_not_create_local_persistence() -> Result<()>
{
    let server = create_mock_responses_server_repeating_assistant("Done").await;
    let agere_home = TempDir::new()?;
    let store_id = Uuid::new_v4().to_string();
    // Plugin startup warmups may create `.tmp` under agere_home. Disable them
    // here so this regression stays focused on thread persistence artifacts.
    create_config_toml_with_thread_store(agere_home.path(), &server.uri(), &store_id)?;

    let loader_overrides = LoaderOverrides::without_managed_config_for_tests();
    let config = ConfigBuilder::default()
        .agere_home(agere_home.path().to_path_buf())
        .fallback_cwd(Some(agere_home.path().to_path_buf()))
        .loader_overrides(loader_overrides.clone())
        .build()
        .await?;

    let thread_store = InMemoryThreadStore::for_id(store_id.clone());
    let _in_memory_store = InMemoryThreadStoreId { store_id };

    let mut client = in_process::start(InProcessStartArgs {
        arg0_paths: Arg0DispatchPaths::default(),
        config: Arc::new(config),
        cli_overrides: Vec::new(),
        loader_overrides,
        thread_config_loader: Arc::new(NoopThreadConfigLoader),
        feedback: AgereFeedback::new(),
        log_db: None,
        state_db: None,
        environment_manager: Arc::new(EnvironmentManager::default_for_tests()),
        config_warnings: Vec::new(),
        session_source: SessionSource::Cli,
        enable_agere_api_key_env: false,
        initialize: InitializeParams {
            client_info: ClientInfo {
                name: "agere-app-server-tests".to_string(),
                title: None,
                version: "0.1.0".to_string(),
            },
            capabilities: None,
        },
        channel_capacity: in_process::DEFAULT_IN_PROCESS_CHANNEL_CAPACITY,
    })
    .await?;

    let response = client
        .request(ClientRequest::ThreadStart {
            request_id: RequestId::Integer(1),
            params: ThreadStartParams::default(),
        })
        .await?
        .expect("thread/start should succeed");
    let ThreadStartResponse { thread, .. } =
        serde_json::from_value(response).expect("thread/start response should parse");
    assert_eq!(thread.path, None);

    client
        .request(ClientRequest::TurnStart {
            request_id: RequestId::Integer(2),
            params: TurnStartParams {
                thread_id: thread.id.clone(),
                input: vec![V2UserInput::Text {
                    text: "Hello".to_string(),
                    text_elements: Vec::new(),
                }],
                ..Default::default()
            },
        })
        .await?
        .expect("turn/start should succeed");

    timeout(DEFAULT_READ_TIMEOUT, async {
        loop {
            let Some(event) = client.next_event().await else {
                anyhow::bail!("in-process app-server stopped before turn/completed");
            };
            if let InProcessServerEvent::ServerNotification(ServerNotification::TurnCompleted(
                completed,
            )) = event
                && completed.thread_id == thread.id
            {
                return Ok::<(), anyhow::Error>(());
            }
        }
    })
    .await??;

    client.shutdown().await?;

    let calls = thread_store.calls().await;
    assert_eq!(calls.create_thread, 1);
    assert!(
        calls.append_items > 0,
        "turn/start should append rollout items through the injected store"
    );
    assert!(
        calls.flush_thread > 0,
        "turn completion should flush through the injected store"
    );

    assert_no_local_persistence_artifacts(agere_home.path())?;

    Ok(())
}

fn assert_no_local_persistence_artifacts(agere_home: &Path) -> Result<()> {
    // These are the observable tripwires for accidental local persistence. If a
    // future code path constructs a local rollout/session store or opens the
    // local thread sqlite database, it should leave one of these artifacts in
    // the isolated test agere_home.
    assert!(
        !agere_home.join("sessions").exists(),
        "non-local thread persistence should not create local rollout sessions"
    );
    assert!(
        !agere_home.join("archived_sessions").exists(),
        "non-local thread persistence should not create archived rollout sessions"
    );
    assert!(
        !agere_state::state_db_path(agere_home).exists(),
        "non-local thread persistence should not create local thread sqlite"
    );

    let sqlite_artifacts = std::fs::read_dir(agere_home)?
        .filter_map(std::result::Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| {
                    name.ends_with(".sqlite")
                        || name.ends_with(".sqlite-shm")
                        || name.ends_with(".sqlite-wal")
                })
        })
        .collect::<Vec<_>>();

    assert!(
        sqlite_artifacts.is_empty(),
        "non-local thread persistence should not create sqlite artifacts: {sqlite_artifacts:?}"
    );
    let mut entries = agere_home_entries(agere_home)?;
    // Bazel test runs may initialize shell snapshot storage under agere_home.
    // That is not thread persistence; keep the assertion focused on rollout,
    // session, sqlite, and other unexpected thread-store artifacts.
    entries.remove("shell_snapshots");
    assert_eq!(
        entries,
        BTreeSet::from([
            "config.toml".to_string(),
            "installation_id".to_string(),
            "memories".to_string(),
            "skills".to_string(),
        ]),
        "non-local thread persistence should not create unexpected files in agere_home"
    );

    Ok(())
}

fn agere_home_entries(agere_home: &Path) -> Result<BTreeSet<String>> {
    Ok(std::fs::read_dir(agere_home)?
        .filter_map(|entry| {
            let entry = entry.ok()?;
            Some(entry.file_name().to_string_lossy().into_owned())
        })
        .collect())
}

struct InMemoryThreadStoreId {
    store_id: String,
}

impl Drop for InMemoryThreadStoreId {
    fn drop(&mut self) {
        InMemoryThreadStore::remove_id(&self.store_id);
    }
}

fn create_config_toml_with_thread_store(
    agere_home: &Path,
    server_uri: &str,
    store_id: &str,
) -> std::io::Result<()> {
    std::fs::write(
        agere_home.join("config.toml"),
        format!(
            r#"
model = "mock-model"
approval_policy = "never"
access_mode = "read-only"
experimental_thread_store = {{ type = "in_memory", id = "{store_id}" }}

model_provider = "mock_provider"

[model_providers.mock_provider]
name = "Mock provider for test"
base_url = "{server_uri}/v1"
wire_api = "responses"
request_max_retries = 0
stream_max_retries = 0

[features]
plugins = false
"#
        ),
    )
}
