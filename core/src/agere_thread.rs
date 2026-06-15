use crate::agent::AgentStatus;
use crate::config::ConstraintResult;
use crate::file_watcher::WatchRegistration;
use crate::goals::GoalRuntimeEvent;
use crate::session::Agere;
use crate::session::SessionSettingsUpdate;
use crate::session::SteerInputError;
use agere_features::Feature;
use agere_model_provider_info::ModelProviderInfo;
use agere_protocol::config_types::ApprovalsReviewer;
use agere_protocol::config_types::CollaborationMode;
use agere_protocol::config_types::Personality;
use agere_protocol::config_types::ReasoningSummary;
use agere_protocol::config_types::ServiceTier;
use agere_protocol::config_types::WindowsExecutionRestrictionLevel;
use agere_protocol::error::AgereErr;
use agere_protocol::error::Result as AgereResult;
use agere_protocol::mcp::CallToolResult;
use agere_protocol::models::ContentItem;
use agere_protocol::models::PermissionProfile;
use agere_protocol::models::ResponseInputItem;
use agere_protocol::models::ResponseItem;
use agere_protocol::openai_models::ReasoningEffort;
use agere_protocol::protocol::AskForApproval;
use agere_protocol::protocol::Event;
use agere_protocol::protocol::Op;
use agere_protocol::protocol::SessionSource;
use agere_protocol::protocol::Submission;
use agere_protocol::protocol::ThreadMemoryMode;
use agere_protocol::protocol::TokenUsageInfo;
use agere_protocol::protocol::W3cTraceContext;
use agere_protocol::user_input::UserInput;
use agere_utils_fs::AbsolutePathBuf;
use rmcp::model::ReadResourceRequestParams;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::sync::watch;

use agere_rollout::state_db::StateDbHandle;

#[derive(Clone, Debug)]
pub struct ThreadConfigSnapshot {
    pub model: String,
    pub model_provider_id: String,
    pub service_tier: Option<ServiceTier>,
    pub approval_policy: AskForApproval,
    pub approvals_reviewer: ApprovalsReviewer,
    pub permission_profile: PermissionProfile,
    pub cwd: AbsolutePathBuf,
    pub ephemeral: bool,
    pub reasoning_effort: Option<ReasoningEffort>,
    pub personality: Option<Personality>,
    pub session_source: SessionSource,
}

impl ThreadConfigSnapshot {
    pub fn access_policy(&self) -> String {
        self.permission_profile
            .to_legacy_access_policy(self.cwd.as_path())
            .unwrap_or_else(|err| panic!("legacy access policy: {err}"))
    }
}

/// Turn context overrides that app-server validates before starting a turn.
#[derive(Clone, Default)]
pub struct AgereThreadTurnContextOverrides {
    pub cwd: Option<PathBuf>,
    pub approval_policy: Option<AskForApproval>,
    pub approvals_reviewer: Option<ApprovalsReviewer>,
    pub permission_profile: Option<PermissionProfile>,
    pub windows_execution_restriction_level: Option<WindowsExecutionRestrictionLevel>,
    pub provider: Option<(String, agere_model_provider_info::ModelProviderInfo)>,
    pub provider_config: Option<std::sync::Arc<crate::config::Config>>,
    pub model: Option<String>,
    pub effort: Option<Option<ReasoningEffort>>,
    pub summary: Option<ReasoningSummary>,
    pub service_tier: Option<Option<ServiceTier>>,
    pub collaboration_mode: Option<CollaborationMode>,
    pub personality: Option<Personality>,
}

pub struct AgereThread {
    pub(crate) agere: Agere,
    pub(crate) session_source: SessionSource,
    rollout_path: Option<PathBuf>,
    out_of_band_elicitation_count: Mutex<u64>,
    _watch_registration: WatchRegistration,
}

/// Conduit for the bidirectional stream of messages that compose a thread
/// (formerly called a conversation) in Agere.
impl AgereThread {
    pub(crate) fn new(
        agere: Agere,
        rollout_path: Option<PathBuf>,
        session_source: SessionSource,
        watch_registration: WatchRegistration,
    ) -> Self {
        Self {
            agere,
            session_source,
            rollout_path,
            out_of_band_elicitation_count: Mutex::new(0),
            _watch_registration: watch_registration,
        }
    }

    pub async fn submit(&self, op: Op) -> AgereResult<String> {
        self.agere.submit(op).await
    }

    pub async fn shutdown_and_wait(&self) -> AgereResult<()> {
        self.agere.shutdown_and_wait().await
    }

    /// Wait until the underlying session loop has terminated.
    pub async fn wait_until_terminated(&self) {
        self.agere.session_loop_termination.clone().await;
    }

    pub async fn apply_goal_resume_runtime_effects(&self) -> anyhow::Result<()> {
        self.agere
            .session
            .goal_runtime_apply(GoalRuntimeEvent::ThreadResumed)
            .await
    }

    pub async fn continue_active_goal_if_idle(&self) -> anyhow::Result<()> {
        self.agere
            .session
            .goal_runtime_apply(GoalRuntimeEvent::MaybeContinueIfIdle)
            .await
    }

    pub async fn prepare_external_goal_mutation(&self) {
        if let Err(err) = self
            .agere
            .session
            .goal_runtime_apply(GoalRuntimeEvent::ExternalMutationStarting)
            .await
        {
            tracing::warn!("failed to prepare external goal mutation: {err}");
        }
    }

    pub async fn apply_external_goal_set(&self, status: agere_state::ThreadGoalStatus) {
        if let Err(err) = self
            .agere
            .session
            .goal_runtime_apply(GoalRuntimeEvent::ExternalSet { status })
            .await
        {
            tracing::warn!("failed to apply external goal status runtime effects: {err}");
        }
    }

    pub async fn apply_external_goal_clear(&self) {
        if let Err(err) = self
            .agere
            .session
            .goal_runtime_apply(GoalRuntimeEvent::ExternalClear)
            .await
        {
            tracing::warn!("failed to apply external goal clear runtime effects: {err}");
        }
    }

    #[doc(hidden)]
    pub async fn ensure_rollout_materialized(&self) {
        self.agere.session.ensure_rollout_materialized().await;
    }

    #[doc(hidden)]
    pub async fn flush_rollout(&self) -> std::io::Result<()> {
        self.agere.session.flush_rollout().await
    }

    pub async fn submit_with_trace(
        &self,
        op: Op,
        trace: Option<W3cTraceContext>,
    ) -> AgereResult<String> {
        self.agere.submit_with_trace(op, trace).await
    }

    /// Persist whether this thread is eligible for future memory generation.
    pub async fn set_thread_memory_mode(&self, mode: ThreadMemoryMode) -> anyhow::Result<()> {
        self.agere.set_thread_memory_mode(mode).await
    }

    pub async fn steer_input(
        &self,
        input: Vec<UserInput>,
        expected_turn_id: Option<&str>,
        responsesapi_client_metadata: Option<HashMap<String, String>>,
    ) -> Result<String, SteerInputError> {
        self.agere
            .steer_input(input, expected_turn_id, responsesapi_client_metadata)
            .await
    }

    pub async fn set_app_server_client_info(
        &self,
        app_server_client_name: Option<String>,
        app_server_client_version: Option<String>,
    ) -> ConstraintResult<()> {
        self.agere
            .set_app_server_client_info(app_server_client_name, app_server_client_version)
            .await
    }

    /// Validate persistent turn context overrides without committing them.
    pub async fn validate_turn_context_overrides(
        &self,
        overrides: AgereThreadTurnContextOverrides,
    ) -> ConstraintResult<()> {
        let AgereThreadTurnContextOverrides {
            cwd,
            approval_policy,
            approvals_reviewer,
            permission_profile,
            windows_execution_restriction_level,
            provider,
            provider_config,
            model,
            effort,
            summary,
            service_tier,
            collaboration_mode,
            personality,
        } = overrides;
        let collaboration_mode = if let Some(collaboration_mode) = collaboration_mode {
            collaboration_mode
        } else {
            self.agere
                .session
                .collaboration_mode()
                .await
                .with_updates(model, effort, /*developer_instructions*/ None)
        };

        let updates = SessionSettingsUpdate {
            cwd,
            approval_policy,
            approvals_reviewer,
            permission_profile,
            windows_execution_restriction_level,
            provider,
            provider_config,
            collaboration_mode: Some(collaboration_mode),
            reasoning_summary: summary,
            service_tier,
            personality,
            ..Default::default()
        };
        self.agere.session.validate_settings(&updates).await
    }

    /// Use sparingly: this is intended to be removed soon.
    pub async fn submit_with_id(&self, sub: Submission) -> AgereResult<()> {
        self.agere.submit_with_id(sub).await
    }

    pub async fn next_event(&self) -> AgereResult<Event> {
        self.agere.next_event().await
    }

    pub async fn agent_status(&self) -> AgentStatus {
        self.agere.agent_status().await
    }

    pub async fn has_active_turn(&self) -> bool {
        self.agere.session.has_active_turn().await
    }

    pub(crate) fn subscribe_status(&self) -> watch::Receiver<AgentStatus> {
        self.agere.agent_status.clone()
    }

    /// Returns the complete token usage snapshot currently cached for this thread.
    ///
    /// This accessor is intentionally narrower than direct session access: it lets
    /// app-server lifecycle paths replay restored usage after resume or fork without
    /// exposing broader session mutation authority. A caller that only reads
    /// `total_token_usage` would drop last-turn usage and make the v2
    /// `thread/tokenUsage/updated` payload incomplete.
    pub async fn token_usage_info(&self) -> Option<TokenUsageInfo> {
        self.agere.session.token_usage_info().await
    }

    /// Records a user-role session-prefix message without creating a new user turn boundary.
    pub(crate) async fn inject_user_message_without_turn(&self, message: String) {
        let message = ResponseItem::Message {
            id: None,
            role: "user".to_string(),
            content: vec![ContentItem::InputText { text: message }],
            phase: None,
        };
        let pending_item = match pending_message_input_item(&message) {
            Ok(pending_item) => pending_item,
            Err(err) => {
                debug_assert!(false, "session-prefix message append should succeed: {err}");
                return;
            }
        };
        if self
            .agere
            .session
            .inject_response_items(vec![pending_item])
            .await
            .is_err()
        {
            let turn_context = self.agere.session.new_default_turn().await;
            self.agere
                .session
                .record_conversation_items(turn_context.as_ref(), &[message])
                .await;
        }
    }

    /// Append a prebuilt message to the thread history without treating it as a user turn.
    ///
    /// If the thread already has an active turn, the message is queued as pending input for that
    /// turn. Otherwise it is queued at session scope and a regular turn is started so the agent
    /// can consume that pending input through the normal turn pipeline.
    #[cfg(test)]
    pub(crate) async fn append_message(&self, message: ResponseItem) -> AgereResult<String> {
        let submission_id = uuid::Uuid::new_v4().to_string();
        let pending_item = pending_message_input_item(&message)?;
        if let Err(items) = self
            .agere
            .session
            .inject_response_items(vec![pending_item])
            .await
        {
            self.agere
                .session
                .queue_response_items_for_next_turn(items)
                .await;
            self.agere.session.maybe_start_turn_for_pending_work().await;
        }

        Ok(submission_id)
    }

    /// Append raw Responses API items to the thread's model-visible history.
    pub async fn inject_response_items(&self, items: Vec<ResponseItem>) -> AgereResult<()> {
        if items.is_empty() {
            return Err(AgereErr::InvalidRequest(
                "items must not be empty".to_string(),
            ));
        }

        let turn_context = self.agere.session.new_default_turn().await;
        if self.agere.session.reference_context_item().await.is_none() {
            self.agere
                .session
                .record_context_updates_and_set_reference_context_item(turn_context.as_ref())
                .await;
        }
        self.agere
            .session
            .record_conversation_items(turn_context.as_ref(), &items)
            .await;
        self.agere.session.flush_rollout().await?;
        Ok(())
    }

    pub fn rollout_path(&self) -> Option<PathBuf> {
        self.rollout_path.clone()
    }

    pub fn state_db(&self) -> Option<StateDbHandle> {
        self.agere.state_db()
    }

    pub async fn config_snapshot(&self) -> ThreadConfigSnapshot {
        self.agere.thread_config_snapshot().await
    }

    /// Stage a fresh provider (resolved by the app-server against the latest
    /// config, so it sees runtime-added providers) for the next turn. The
    /// protocol `Op` can only carry the provider id; the full
    /// `ModelProviderInfo` rides this staging slot.
    pub async fn stage_model_provider(
        &self,
        submission_id: String,
        provider_id: String,
        provider: ModelProviderInfo,
        config: Arc<crate::config::Config>,
    ) {
        self.agere
            .session
            .stage_model_provider(submission_id, provider_id, provider, config);
    }

    pub async fn config(&self) -> Arc<crate::config::Config> {
        self.agere.session.get_config().await
    }

    pub async fn read_mcp_resource(
        &self,
        server: &str,
        uri: &str,
    ) -> anyhow::Result<serde_json::Value> {
        let result = self
            .agere
            .session
            .read_resource(
                server,
                ReadResourceRequestParams {
                    meta: None,
                    uri: uri.to_string(),
                },
            )
            .await?;

        Ok(serde_json::to_value(result)?)
    }

    pub async fn call_mcp_tool(
        &self,
        server: &str,
        tool: &str,
        arguments: Option<serde_json::Value>,
        meta: Option<serde_json::Value>,
    ) -> anyhow::Result<CallToolResult> {
        self.agere
            .session
            .call_tool(server, tool, arguments, meta)
            .await
    }

    pub fn enabled(&self, feature: Feature) -> bool {
        self.agere.enabled(feature)
    }

    pub async fn increment_out_of_band_elicitation_count(&self) -> AgereResult<u64> {
        let mut guard = self.out_of_band_elicitation_count.lock().await;
        let was_zero = *guard == 0;
        *guard = guard.checked_add(1).ok_or_else(|| {
            AgereErr::Fatal("out-of-band elicitation count overflowed".to_string())
        })?;

        if was_zero {
            self.agere
                .session
                .set_out_of_band_elicitation_pause_state(/*paused*/ true);
        }

        Ok(*guard)
    }

    pub async fn decrement_out_of_band_elicitation_count(&self) -> AgereResult<u64> {
        let mut guard = self.out_of_band_elicitation_count.lock().await;
        if *guard == 0 {
            return Err(AgereErr::InvalidRequest(
                "out-of-band elicitation count is already zero".to_string(),
            ));
        }

        *guard -= 1;
        let now_zero = *guard == 0;
        if now_zero {
            self.agere
                .session
                .set_out_of_band_elicitation_pause_state(/*paused*/ false);
        }

        Ok(*guard)
    }
}

fn pending_message_input_item(message: &ResponseItem) -> AgereResult<ResponseInputItem> {
    match message {
        ResponseItem::Message {
            role,
            content,
            phase,
            ..
        } => Ok(ResponseInputItem::Message {
            role: role.clone(),
            content: content.clone(),
            phase: phase.clone(),
        }),
        _ => Err(AgereErr::InvalidRequest(
            "append_message only supports ResponseItem::Message".to_string(),
        )),
    }
}
