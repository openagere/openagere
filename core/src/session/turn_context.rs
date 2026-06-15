use super::*;
use crate::access_policy_transforms::effective_file_system_access_policy;
use crate::access_policy_transforms::effective_network_access_policy;
use crate::config::GhostSnapshotConfig;
use agere_model_provider::SharedModelProvider;
use agere_model_provider::create_model_provider;
use agere_model_provider_info::BUILT_IN_PROVIDERS;
use agere_protocol::models::AdditionalPermissionProfile;
use agere_protocol::openai_models::ApplyPatchToolType;
use agere_protocol::protocol::TurnEnvironmentSelection;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;

pub(super) fn image_generation_tool_auth_allowed(auth_manager: Option<&AuthManager>) -> bool {
    auth_manager.is_some_and(AuthManager::current_auth_uses_agere_backend)
}

#[derive(Clone, Debug)]
pub(crate) struct TurnSkillsContext {
    pub(crate) outcome: Arc<SkillLoadOutcome>,
    pub(crate) implicit_invocation_seen_skills: Arc<Mutex<HashSet<String>>>,
}

impl TurnSkillsContext {
    pub(crate) fn new(outcome: Arc<SkillLoadOutcome>) -> Self {
        Self {
            outcome,
            implicit_invocation_seen_skills: Arc::new(Mutex::new(HashSet::new())),
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct TurnEnvironment {
    pub(crate) environment_id: String,
    pub(crate) environment: Arc<Environment>,
    pub(crate) cwd: AbsolutePathBuf,
}

impl TurnEnvironment {
    pub(crate) fn selection(&self) -> TurnEnvironmentSelection {
        TurnEnvironmentSelection {
            environment_id: self.environment_id.clone(),
            cwd: self.cwd.clone(),
        }
    }
}

/// The context needed for a single turn of the thread.
pub(crate) struct TurnContext {
    pub(crate) sub_id: String,
    pub(crate) trace_id: Option<String>,
    pub(crate) realtime_active: bool,
    pub(crate) config: Arc<Config>,
    pub(crate) auth_manager: Option<Arc<AuthManager>>,
    pub(crate) model_info: ModelInfo,
    pub(crate) session_telemetry: SessionTelemetry,
    pub(crate) provider: SharedModelProvider,
    pub(crate) reasoning_effort: Option<ReasoningEffortConfig>,
    pub(crate) reasoning_summary: ReasoningSummaryConfig,
    pub(crate) session_source: SessionSource,
    pub(crate) environment: Option<Arc<Environment>>,
    pub(crate) local_fs: Arc<dyn agere_exec_server::ExecutorFileSystem>,
    pub(crate) environments: Vec<TurnEnvironment>,
    /// The session's absolute working directory. All relative paths provided
    /// by the model as well as filesystem access policy are resolved against this path
    /// instead of `std::env::current_dir()`.
    pub(crate) cwd: AbsolutePathBuf,
    pub(crate) current_date: Option<String>,
    pub(crate) timezone: Option<String>,
    pub(crate) app_server_client_name: Option<String>,
    pub(crate) developer_instructions: Option<String>,
    pub(crate) compact_prompt: Option<String>,
    pub(crate) user_instructions: Option<String>,
    pub(crate) collaboration_mode: CollaborationMode,
    pub(crate) personality: Option<Personality>,
    pub(crate) approval_policy: Constrained<AskForApproval>,
    pub(crate) permission_profile: PermissionProfile,
    pub(crate) network: Option<NetworkProxy>,
    pub(crate) windows_execution_restriction_level: WindowsExecutionRestrictionLevel,
    pub(crate) shell_environment_policy: ShellEnvironmentPolicy,
    pub(crate) tools_config: ToolsConfig,
    pub(crate) features: ManagedFeatures,
    pub(crate) ghost_snapshot: GhostSnapshotConfig,
    pub(crate) final_output_json_schema: Option<Value>,
    pub(crate) agere_self_exe: Option<PathBuf>,
    pub(crate) agere_linux_exe: Option<PathBuf>,
    pub(crate) tool_call_gate: Arc<ReadinessFlag>,
    pub(crate) truncation_policy: TruncationPolicy,
    pub(crate) dynamic_tools: Vec<DynamicToolSpec>,
    pub(crate) turn_metadata_state: Arc<TurnMetadataState>,
    pub(crate) turn_skills: TurnSkillsContext,
    pub(crate) turn_timing_state: Arc<TurnTimingState>,
    pub(crate) server_model_warning_emitted: AtomicBool,
    pub(crate) model_verification_emitted: AtomicBool,
    pub(crate) provider_changed_for_turn: bool,
}
impl TurnContext {
    pub(crate) fn permission_profile(&self) -> PermissionProfile {
        self.permission_profile.clone()
    }

    pub(crate) fn file_system_access_policy(&self) -> FileSystemAccessPolicy {
        self.permission_profile.file_system_access_policy()
    }

    pub(crate) fn network_access_policy(&self) -> NetworkAccessPolicy {
        self.permission_profile.network_access_policy()
    }

    pub(crate) fn local_filesystem(&self) -> Arc<dyn agere_exec_server::ExecutorFileSystem> {
        Arc::clone(&self.local_fs)
    }

    pub(crate) fn access_policy(&self) -> String {
        self.permission_profile
            .to_legacy_access_policy(self.cwd.as_path())
            .unwrap_or_else(|err| panic!("legacy access policy: {err}"))
    }

    /// Returns the effective context window after applying the usable percentage.
    /// Used for compaction and input size estimation.
    pub(crate) fn model_context_window(&self) -> Option<i64> {
        let effective_context_window_percent = self.model_info.effective_context_window_percent;
        self.model_info
            .resolved_context_window()
            .map(|context_window| {
                context_window.saturating_mul(effective_context_window_percent) / 100
            })
    }

    pub(crate) fn apps_enabled(&self) -> bool {
        let uses_agere_backend = self
            .auth_manager
            .as_deref()
            .is_some_and(AuthManager::current_auth_uses_agere_backend);
        self.features.apps_enabled_for_auth(uses_agere_backend)
    }

    pub(crate) async fn with_model(
        &self,
        model: String,
        models_manager: &SharedModelsManager,
    ) -> Self {
        let mut config = (*self.config).clone();
        config.model = Some(model.clone());
        let mut model_info = models_manager
            .get_model_info(model.as_str(), &config.to_models_manager_config())
            .await;

        // Same inference logic as in make_turn_context.
        if model_info.apply_patch_tool_type.is_none() {
            model_info.apply_patch_tool_type =
                if self.provider.info().supports_apply_patch_freeform() {
                    Some(ApplyPatchToolType::Freeform)
                } else {
                    Some(ApplyPatchToolType::Function)
                };
        }
        let truncation_policy = model_info.truncation_policy.into();
        let supported_reasoning_levels = model_info
            .supported_reasoning_levels
            .iter()
            .map(|preset| preset.effort)
            .collect::<Vec<_>>();
        let reasoning_effort = if let Some(current_reasoning_effort) = self.reasoning_effort {
            if supported_reasoning_levels.contains(&current_reasoning_effort) {
                Some(current_reasoning_effort)
            } else {
                supported_reasoning_levels
                    .get(supported_reasoning_levels.len().saturating_sub(1) / 2)
                    .copied()
                    .or(model_info.default_reasoning_level)
            }
        } else {
            supported_reasoning_levels
                .get(supported_reasoning_levels.len().saturating_sub(1) / 2)
                .copied()
                .or(model_info.default_reasoning_level)
        };
        config.model_reasoning_effort = reasoning_effort;

        let collaboration_mode = self.collaboration_mode.with_updates(
            Some(model.clone()),
            Some(reasoning_effort),
            /*developer_instructions*/ None,
        );
        let features = self.features.clone();
        let provider_capabilities = self.provider.capabilities();
        let tools_config = ToolsConfig::new(&ToolsConfigParams {
            model_info: &model_info,
            available_models: &models_manager
                .list_models(RefreshStrategy::OnlineIfUncached)
                .await,
            features: &features,
            image_generation_tool_auth_allowed: image_generation_tool_auth_allowed(
                self.auth_manager.as_deref(),
            ),
            web_search_mode: self.tools_config.web_search_mode,
            session_source: self.session_source.clone(),
            permission_profile: &self.permission_profile,
            windows_execution_restriction_level: self.windows_execution_restriction_level,
        })
        .with_namespace_tools_capability(provider_capabilities.namespace_tools)
        .with_image_generation_capability(provider_capabilities.image_generation)
        .with_web_search_capability(provider_capabilities.web_search)
        .with_unified_exec_shell_mode(self.tools_config.unified_exec_shell_mode.clone())
        .with_web_search_config(self.tools_config.web_search_config.clone())
        .with_allow_login_shell(self.tools_config.allow_login_shell)
        .with_spawn_agent_usage_hint(config.multi_agent_v2.usage_hint_enabled)
        .with_spawn_agent_usage_hint_text(config.multi_agent_v2.usage_hint_text.clone())
        .with_hide_spawn_agent_metadata(config.multi_agent_v2.hide_spawn_agent_metadata)
        .with_goal_tools_allowed(self.tools_config.goal_tools)
        .with_max_concurrent_threads_per_session(
            config
                .features
                .enabled(Feature::MultiAgentV2)
                .then_some(config.multi_agent_v2.max_concurrent_threads_per_session),
        )
        .with_wait_agent_min_timeout_ms(
            config
                .features
                .enabled(Feature::MultiAgentV2)
                .then_some(config.multi_agent_v2.min_wait_timeout_ms),
        )
        .with_agent_type_description(crate::agent::role::spawn_tool_spec::build(
            &config.agent_roles,
        ));

        Self {
            sub_id: self.sub_id.clone(),
            trace_id: self.trace_id.clone(),
            realtime_active: self.realtime_active,
            config: Arc::new(config),
            auth_manager: self.auth_manager.clone(),
            model_info: model_info.clone(),
            session_telemetry: self
                .session_telemetry
                .clone()
                .with_model(model.as_str(), model_info.slug.as_str()),
            provider: self.provider.clone(),
            reasoning_effort,
            reasoning_summary: self.reasoning_summary,
            session_source: self.session_source.clone(),
            environment: self.environment.clone(),
            local_fs: Arc::clone(&self.local_fs),
            environments: self.environments.clone(),
            cwd: self.cwd.clone(),
            current_date: self.current_date.clone(),
            timezone: self.timezone.clone(),
            app_server_client_name: self.app_server_client_name.clone(),
            developer_instructions: self.developer_instructions.clone(),
            compact_prompt: self.compact_prompt.clone(),
            user_instructions: self.user_instructions.clone(),
            collaboration_mode,
            personality: self.personality,
            approval_policy: self.approval_policy.clone(),
            permission_profile: self.permission_profile.clone(),
            network: self.network.clone(),
            windows_execution_restriction_level: self.windows_execution_restriction_level,
            shell_environment_policy: self.shell_environment_policy.clone(),
            tools_config,
            features,
            ghost_snapshot: self.ghost_snapshot.clone(),
            final_output_json_schema: self.final_output_json_schema.clone(),
            agere_self_exe: self.agere_self_exe.clone(),
            agere_linux_exe: self.agere_linux_exe.clone(),
            tool_call_gate: Arc::new(ReadinessFlag::new()),
            truncation_policy,
            dynamic_tools: self.dynamic_tools.clone(),
            turn_metadata_state: self.turn_metadata_state.clone(),
            turn_skills: self.turn_skills.clone(),
            turn_timing_state: Arc::clone(&self.turn_timing_state),
            server_model_warning_emitted: AtomicBool::new(
                self.server_model_warning_emitted.load(Ordering::Relaxed),
            ),
            model_verification_emitted: AtomicBool::new(
                self.model_verification_emitted.load(Ordering::Relaxed),
            ),
            provider_changed_for_turn: self.provider_changed_for_turn,
        }
    }

    pub(crate) fn resolve_path(&self, path: Option<String>) -> AbsolutePathBuf {
        path.as_ref()
            .map_or_else(|| self.cwd.clone(), |path| self.cwd.join(path))
    }

    pub(crate) fn file_system_access_context(
        &self,
        additional_permissions: Option<AdditionalPermissionProfile>,
    ) -> FileSystemAccessContext {
        let (base_file_system_access_policy, base_network_access_policy) =
            self.permission_profile.to_runtime_permissions();
        let file_system_access_policy = effective_file_system_access_policy(
            &base_file_system_access_policy,
            additional_permissions.as_ref(),
        );
        let network_access_policy = effective_network_access_policy(
            base_network_access_policy,
            additional_permissions.as_ref(),
        );
        let permissions = PermissionProfile::from_runtime_permissions_with_filesystem_isolation(
            self.permission_profile.filesystem_isolation(),
            &file_system_access_policy,
            network_access_policy,
        );
        FileSystemAccessContext {
            permissions,
            cwd: Some(self.cwd.clone()),
            windows_execution_restriction_level: self.windows_execution_restriction_level,
            windows_execution_private_desktop: self
                .config
                .permissions
                .windows_execution_private_desktop,
            use_legacy_landlock: self.features.use_legacy_landlock(),
        }
    }

    fn non_legacy_file_system_access_policy(&self) -> Option<FileSystemAccessPolicy> {
        // Omit the derived split filesystem policy when it is equivalent to
        // the legacy access-policy projection. This keeps turn-context payloads stable
        // while both fields exist; once callers consume only the split policy,
        // this comparison and the legacy projection should go away.
        let legacy_file_system_access_policy =
            FileSystemAccessPolicy::from_legacy_access_policy_for_cwd(
                &self.access_policy(),
                &self.cwd,
            );
        let file_system_access_policy = self.file_system_access_policy();
        (file_system_access_policy != legacy_file_system_access_policy)
            .then_some(file_system_access_policy)
    }

    pub(crate) fn compact_prompt(&self) -> &str {
        self.compact_prompt
            .as_deref()
            .unwrap_or(compact::SUMMARIZATION_PROMPT)
    }

    pub(crate) fn to_turn_context_item(&self) -> TurnContextItem {
        TurnContextItem {
            turn_id: Some(self.sub_id.clone()),
            trace_id: self.trace_id.clone(),
            cwd: self.cwd.to_path_buf(),
            current_date: self.current_date.clone(),
            timezone: self.timezone.clone(),
            approval_policy: self.approval_policy.value(),
            permission_profile: Some(self.permission_profile()),
            network: self.turn_context_network_item(),
            file_system_access_policy: self.non_legacy_file_system_access_policy(),
            model: self.model_info.slug.clone(),
            personality: self.personality,
            collaboration_mode: Some(self.collaboration_mode.clone()),
            realtime_active: Some(self.realtime_active),
            effort: self.reasoning_effort,
            summary: self.reasoning_summary,
            user_instructions: self.user_instructions.clone(),
            developer_instructions: self.developer_instructions.clone(),
            final_output_json_schema: self.final_output_json_schema.clone(),
            truncation_policy: Some(self.truncation_policy),
        }
    }

    fn turn_context_network_item(&self) -> Option<TurnContextNetworkItem> {
        let network = self
            .config
            .config_layer_stack
            .requirements()
            .network
            .as_ref()?;
        Some(TurnContextNetworkItem {
            allowed_domains: network
                .domains
                .as_ref()
                .and_then(agere_config::NetworkDomainPermissionsToml::allowed_domains)
                .unwrap_or_default(),
            denied_domains: network
                .domains
                .as_ref()
                .and_then(agere_config::NetworkDomainPermissionsToml::denied_domains)
                .unwrap_or_default(),
        })
    }
}

fn local_time_context() -> (String, String) {
    match iana_time_zone::get_timezone() {
        Ok(timezone) => (Local::now().format("%Y-%m-%d").to_string(), timezone),
        Err(_) => (
            Utc::now().format("%Y-%m-%d").to_string(),
            "Etc/UTC".to_string(),
        ),
    }
}

impl Session {
    /// Don't expand the number of mutated arguments on config. We are in the process of getting rid of it.
    pub(crate) fn build_per_turn_config(
        session_configuration: &SessionConfiguration,
        cwd: AbsolutePathBuf,
    ) -> Config {
        // todo(aibrahim): store this state somewhere else so we don't need to mut config
        let config = session_configuration.original_config_do_not_use.clone();
        let mut per_turn_config = (*config).clone();
        per_turn_config.cwd = cwd;
        per_turn_config.model_provider_id = session_configuration.provider_id.clone();
        per_turn_config.model_provider = session_configuration.provider.clone();
        per_turn_config.model_reasoning_effort =
            session_configuration.collaboration_mode.reasoning_effort();
        per_turn_config.model_reasoning_summary = session_configuration.model_reasoning_summary;
        per_turn_config.service_tier = session_configuration.service_tier;
        per_turn_config.personality = session_configuration.personality;
        per_turn_config.approvals_reviewer = session_configuration.approvals_reviewer;
        per_turn_config.permissions.permission_profile =
            session_configuration.permission_profile.clone();
        let permission_profile = session_configuration.permission_profile();
        let resolved_web_search_mode =
            resolve_web_search_mode_for_turn(&per_turn_config.web_search_mode, &permission_profile);
        if let Err(err) = per_turn_config
            .web_search_mode
            .set(resolved_web_search_mode)
        {
            let fallback_value = per_turn_config.web_search_mode.value();
            tracing::warn!(
                error = %err,
                ?resolved_web_search_mode,
                ?fallback_value,
                "resolved web_search_mode is disallowed by requirements; keeping constrained value"
            );
        }
        per_turn_config.features = config.features.clone();
        per_turn_config
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn make_turn_context(
        conversation_id: ThreadId,
        auth_manager: Option<Arc<AuthManager>>,
        session_telemetry: &SessionTelemetry,
        provider: ModelProviderInfo,
        session_configuration: &SessionConfiguration,
        user_shell: &shell::Shell,
        shell_zsh_path: Option<&PathBuf>,
        main_execve_wrapper_exe: Option<&PathBuf>,
        per_turn_config: Config,
        model_info: ModelInfo,
        models_manager: &SharedModelsManager,
        network: Option<NetworkProxy>,
        environment: Option<Arc<Environment>>,
        environments: Vec<TurnEnvironment>,
        cwd: AbsolutePathBuf,
        sub_id: String,
        skills_outcome: Arc<SkillLoadOutcome>,
        goal_tools_supported: bool,
    ) -> TurnContext {
        let reasoning_effort = session_configuration.collaboration_mode.reasoning_effort();
        let reasoning_summary = session_configuration
            .model_reasoning_summary
            .unwrap_or(model_info.default_reasoning_summary);
        let session_telemetry = session_telemetry.clone().with_model(
            session_configuration.collaboration_mode.model(),
            model_info.slug.as_str(),
        );
        let session_source = session_configuration.session_source.clone();
        let image_generation_tool_auth_allowed =
            image_generation_tool_auth_allowed(auth_manager.as_deref());
        let auth_manager_for_context = auth_manager.clone();

        // Infer apply_patch_tool_type from provider when not explicitly set.
        // Freeform (Lark grammar) is only supported by OpenAI Responses API
        // and compatible providers. All others get Function variant.
        let mut model_info = model_info;
        if model_info.apply_patch_tool_type.is_none() {
            model_info.apply_patch_tool_type = if provider.supports_apply_patch_freeform() {
                Some(ApplyPatchToolType::Freeform)
            } else {
                Some(ApplyPatchToolType::Function)
            };
        }

        let provider_for_context = create_model_provider(provider, auth_manager, vec![]);
        let provider_capabilities = provider_for_context.capabilities();
        let session_telemetry_for_context = session_telemetry;
        let tools_config = ToolsConfig::new(&ToolsConfigParams {
            model_info: &model_info,
            available_models: &models_manager.try_list_models().unwrap_or_default(),
            features: &per_turn_config.features,
            image_generation_tool_auth_allowed,
            web_search_mode: Some(per_turn_config.web_search_mode.value()),
            session_source: session_source.clone(),
            permission_profile: &session_configuration.permission_profile(),
            windows_execution_restriction_level: session_configuration
                .windows_execution_restriction_level,
        })
        .with_namespace_tools_capability(provider_capabilities.namespace_tools)
        .with_image_generation_capability(provider_capabilities.image_generation)
        .with_web_search_capability(provider_capabilities.web_search)
        .with_unified_exec_shell_mode_for_session(
            crate::tools::spec::tool_user_shell_type(user_shell),
            shell_zsh_path,
            main_execve_wrapper_exe,
        )
        .with_web_search_config(per_turn_config.web_search_config.clone())
        .with_allow_login_shell(per_turn_config.permissions.allow_login_shell)
        .with_spawn_agent_usage_hint(per_turn_config.multi_agent_v2.usage_hint_enabled)
        .with_spawn_agent_usage_hint_text(per_turn_config.multi_agent_v2.usage_hint_text.clone())
        .with_hide_spawn_agent_metadata(per_turn_config.multi_agent_v2.hide_spawn_agent_metadata)
        .with_goal_tools_allowed(goal_tools_supported)
        .with_max_concurrent_threads_per_session(
            per_turn_config
                .features
                .enabled(Feature::MultiAgentV2)
                .then_some(
                    per_turn_config
                        .multi_agent_v2
                        .max_concurrent_threads_per_session,
                ),
        )
        .with_wait_agent_min_timeout_ms(
            per_turn_config
                .features
                .enabled(Feature::MultiAgentV2)
                .then_some(per_turn_config.multi_agent_v2.min_wait_timeout_ms),
        )
        .with_agent_type_description(crate::agent::role::spawn_tool_spec::build(
            &per_turn_config.agent_roles,
        ));

        let per_turn_config = Arc::new(per_turn_config);
        let turn_metadata_state = Arc::new(TurnMetadataState::new(
            conversation_id.to_string(),
            &session_source,
            sub_id.clone(),
            cwd.clone(),
            &session_configuration.permission_profile(),
            session_configuration.windows_execution_restriction_level,
            network.is_some(),
        ));
        let (current_date, timezone) = local_time_context();
        TurnContext {
            sub_id,
            trace_id: current_span_trace_id(),
            realtime_active: false,
            config: per_turn_config.clone(),
            auth_manager: auth_manager_for_context,
            model_info: model_info.clone(),
            session_telemetry: session_telemetry_for_context,
            provider: provider_for_context,
            reasoning_effort,
            reasoning_summary,
            session_source,
            environment,
            local_fs: Arc::new(agere_exec_server::LocalFileSystem::new()),
            environments,
            cwd,
            current_date: Some(current_date),
            timezone: Some(timezone),
            app_server_client_name: session_configuration.app_server_client_name.clone(),
            developer_instructions: session_configuration.developer_instructions.clone(),
            compact_prompt: session_configuration.compact_prompt.clone(),
            user_instructions: session_configuration.user_instructions.clone(),
            collaboration_mode: session_configuration.collaboration_mode.clone(),
            personality: session_configuration.personality,
            approval_policy: session_configuration.approval_policy.clone(),
            permission_profile: session_configuration.permission_profile(),
            network,
            windows_execution_restriction_level: session_configuration
                .windows_execution_restriction_level,
            shell_environment_policy: per_turn_config.permissions.shell_environment_policy.clone(),
            tools_config,
            features: per_turn_config.features.clone(),
            ghost_snapshot: per_turn_config.ghost_snapshot.clone(),
            final_output_json_schema: None,
            agere_self_exe: per_turn_config.agere_self_exe.clone(),
            agere_linux_exe: per_turn_config.agere_linux_exe.clone(),
            tool_call_gate: Arc::new(ReadinessFlag::new()),
            truncation_policy: model_info.truncation_policy.into(),
            dynamic_tools: session_configuration.dynamic_tools.clone(),
            turn_metadata_state,
            turn_skills: TurnSkillsContext::new(skills_outcome),
            turn_timing_state: Arc::new(TurnTimingState::default()),
            server_model_warning_emitted: AtomicBool::new(false),
            model_verification_emitted: AtomicBool::new(false),
            provider_changed_for_turn: false,
        }
    }

    pub(crate) async fn new_turn_with_sub_id(
        self: &Arc<Self>,
        sub_id: String,
        updates: SessionSettingsUpdate,
    ) -> AgereResult<Arc<TurnContext>> {
        let update_result: AgereResult<_> = {
            let state = self.state.lock().await;
            match state.session_configuration.clone().apply(&updates) {
                Ok(next) => {
                    let effective_environments = updates
                        .environments
                        .clone()
                        .unwrap_or_else(|| next.environments.clone());
                    let turn_environments =
                        self.resolve_turn_environments(&effective_environments)?;
                    let previous_cwd = state.session_configuration.cwd.clone();
                    let previous_permission_profile =
                        state.session_configuration.permission_profile();
                    let next_permission_profile = next.permission_profile();
                    let permission_profile_changed =
                        previous_permission_profile != next_permission_profile;
                    // A provider switch must also reach the session-level wire client /
                    // models manager (see `apply_provider_to_runtime`); writing it only to
                    // `session_configuration` leaves requests on the old base_url.
                    let previous_session_configuration = state.session_configuration.clone();
                    let previous_provider_id = previous_session_configuration.provider_id.clone();
                    let provider_to_apply = if next.provider_id != previous_provider_id {
                        Some((next.provider.clone(), updates.provider_config.clone()))
                    } else {
                        None
                    };
                    let agere_home = next.agere_home.clone();
                    let session_source = next.session_source.clone();
                    // The new configuration is committed below, after any previous-provider
                    // pre-switch compaction has run. Keeping the session on the previous
                    // configuration until then lets that compaction reuse the normal turn /
                    // runtime path instead of a separately constructed previous-provider context.
                    Ok((
                        next,
                        turn_environments,
                        permission_profile_changed,
                        previous_cwd,
                        agere_home,
                        session_source,
                        provider_to_apply,
                        previous_session_configuration,
                    ))
                }
                Err(err) => Err(AgereErr::InvalidRequest(err.to_string())),
            }
        };

        let (
            session_configuration,
            turn_environments,
            permission_profile_changed,
            previous_cwd,
            agere_home,
            session_source,
            provider_to_apply,
            previous_session_configuration,
        ) = match update_result {
            Ok(update) => update,
            Err(err) => {
                let message = err.to_string();
                self.send_event_raw(Event {
                    id: sub_id.clone(),
                    msg: EventMsg::Error(ErrorEvent {
                        message: message.clone(),
                        agere_error_info: Some(AgereErrorInfo::BadRequest),
                    }),
                })
                .await;
                return Err(AgereErr::InvalidRequest(message));
            }
        };

        // Prefer compacting oversized history with the previous provider before the
        // runtime switch. The configuration commit and runtime swap are still deferred
        // here, so the session, its wire client, and its models manager all still target
        // the previous provider: the compaction below runs through the normal turn path
        // on the previous provider. If it fails or still leaves history over the new
        // limit, the normal pre-turn compaction after the switch falls back to the new
        // provider.
        let provider_changed_for_turn = provider_to_apply.is_some();
        if provider_to_apply.is_some() {
            self.maybe_precompact_before_provider_downshift(
                sub_id.as_str(),
                &previous_session_configuration,
                &session_configuration,
                updates.final_output_json_schema.clone(),
                &turn_environments,
            )
            .await;
        }

        // Commit the new configuration, then swap the session runtime so requests reach
        // the new provider's base_url / models manager.
        {
            let mut state = self.state.lock().await;
            state.session_configuration = session_configuration.clone();
        }
        if let Some((provider, config)) = provider_to_apply {
            self.apply_provider_to_runtime(provider, config).await;
        }

        self.maybe_refresh_shell_snapshot_for_cwd(
            &previous_cwd,
            &session_configuration.cwd,
            &agere_home,
            &session_source,
        );

        if permission_profile_changed {
            self.refresh_managed_network_proxy_for_current_permission_profile()
                .await;
        }

        let mut turn_context = self
            .new_turn_from_configuration(
                sub_id,
                session_configuration,
                updates.final_output_json_schema,
                turn_environments,
            )
            .await;
        if let Some(turn_context_mut) = Arc::get_mut(&mut turn_context) {
            turn_context_mut.provider_changed_for_turn = provider_changed_for_turn;
        }
        Ok(turn_context)
    }

    async fn maybe_precompact_before_provider_downshift(
        self: &Arc<Self>,
        sub_id: &str,
        previous_session_configuration: &SessionConfiguration,
        next_session_configuration: &SessionConfiguration,
        final_output_json_schema: Option<Option<Value>>,
        turn_environments: &[TurnEnvironment],
    ) {
        if previous_session_configuration.provider_id == next_session_configuration.provider_id {
            return;
        }
        let total_usage_tokens = self.get_total_token_usage().await;
        let old_context_window = match self
            .previous_turn_settings()
            .await
            .and_then(|settings| settings.context_window)
            .or(previous_session_configuration
                .original_config_do_not_use
                .model_context_window)
        {
            Some(window) => window,
            None => return,
        };
        let next_cwd = turn_environments
            .first()
            .map(|turn_environment| turn_environment.cwd.clone())
            .unwrap_or_else(|| next_session_configuration.cwd.clone());
        let next_per_turn_config =
            Self::build_per_turn_config(next_session_configuration, next_cwd);
        let next_models_manager = self
            .build_provider_models_manager(
                &next_session_configuration.provider,
                Some(Arc::clone(
                    &next_session_configuration.original_config_do_not_use,
                )),
            )
            .await;
        let next_model_info = next_models_manager
            .get_model_info(
                next_session_configuration.collaboration_mode.model(),
                &next_per_turn_config.to_models_manager_config(),
            )
            .await;
        let Some(new_context_window) = next_model_info.resolved_context_window().map(|window| {
            window.saturating_mul(next_model_info.effective_context_window_percent) / 100
        }) else {
            return;
        };
        let new_auto_compact_limit = next_model_info
            .auto_compact_token_limit()
            .unwrap_or(i64::MAX);
        let should_run = super::turn::should_run_downshift_compaction(
            total_usage_tokens,
            new_auto_compact_limit,
            previous_session_configuration.collaboration_mode.model(),
            next_session_configuration.collaboration_mode.model(),
            previous_session_configuration.provider_id.as_str(),
            next_session_configuration.provider_id.as_str(),
            old_context_window,
            new_context_window,
        );
        if !should_run {
            return;
        }

        let precompact_sub_id = format!("{sub_id}-precompact");
        let previous_turn_context = self
            .new_turn_from_configuration(
                precompact_sub_id,
                previous_session_configuration.clone(),
                final_output_json_schema,
                turn_environments.to_vec(),
            )
            .await;
        // The configuration commit and runtime swap are deferred until this returns, so
        // `self.services.model_client` and the session models manager still target the
        // previous provider. The normal compaction path therefore issues its requests on
        // the previous provider without a separately constructed client.
        if let Err(err) = super::turn::run_auto_compact(
            self,
            &previous_turn_context,
            crate::compact::InitialContextInjection::DoNotInject,
            agere_analytics::CompactionReason::ModelDownshift,
            agere_analytics::CompactionPhase::PreTurn,
        )
        .await
        {
            tracing::warn!(
                error = %err,
                previous_provider = %previous_session_configuration.provider_id,
                next_provider = %next_session_configuration.provider_id,
                "previous provider pre-switch compaction failed; falling back to post-switch compaction"
            );
        }
    }

    fn resolve_turn_environments(
        &self,
        environments: &[TurnEnvironmentSelection],
    ) -> AgereResult<Vec<TurnEnvironment>> {
        let mut turn_environments = Vec::with_capacity(environments.len());
        for selected_environment in environments {
            let environment_id = selected_environment.environment_id.clone();
            let environment = self
                .services
                .environment_manager
                .get_environment(&environment_id)
                .ok_or_else(|| {
                    AgereErr::InvalidRequest(format!(
                        "unknown turn environment id `{environment_id}`"
                    ))
                })?;
            let cwd = selected_environment.cwd.clone();
            turn_environments.push(TurnEnvironment {
                environment_id,
                environment,
                cwd,
            });
        }

        Ok(turn_environments)
    }

    async fn new_turn_from_configuration(
        &self,
        sub_id: String,
        session_configuration: SessionConfiguration,
        final_output_json_schema: Option<Option<Value>>,
        turn_environments: Vec<TurnEnvironment>,
    ) -> Arc<TurnContext> {
        let primary_turn_environment = turn_environments.first();
        let environment = primary_turn_environment
            .map(|turn_environment| Arc::clone(&turn_environment.environment));
        let cwd = primary_turn_environment
            .map(|turn_environment| turn_environment.cwd.clone())
            .unwrap_or_else(|| session_configuration.cwd.clone());
        let per_turn_config = Self::build_per_turn_config(&session_configuration, cwd.clone());
        {
            let mcp_connection_manager = self.services.mcp_connection_manager.read().await;
            mcp_connection_manager.set_approval_policy(&session_configuration.approval_policy);
            mcp_connection_manager
                .set_permission_profile(session_configuration.permission_profile());
        }

        let model_info = self
            .services
            .models_manager
            .get_model_info(
                session_configuration.collaboration_mode.model(),
                &per_turn_config.to_models_manager_config(),
            )
            .await;
        let plugin_outcome = self
            .services
            .plugins_manager
            .plugins_for_config(&per_turn_config)
            .await;
        let effective_skill_roots = plugin_outcome.effective_skill_roots();
        let skills_input = skills_load_input_from_config(&per_turn_config, effective_skill_roots);
        let fs = environment
            .as_ref()
            .map(|environment| environment.get_filesystem());
        let skills_outcome = Arc::new(
            self.services
                .skills_manager
                .skills_for_config(&skills_input, fs)
                .await,
        );
        let goal_tools_supported = !per_turn_config.ephemeral && self.state_db().is_some();
        let mut turn_context: TurnContext = Self::make_turn_context(
            self.conversation_id,
            Some(Arc::clone(&self.services.auth_manager)),
            &self.services.session_telemetry,
            session_configuration.provider.clone(),
            &session_configuration,
            self.services.user_shell.as_ref(),
            self.services.shell_zsh_path.as_ref(),
            self.services.main_execve_wrapper_exe.as_ref(),
            per_turn_config,
            model_info,
            &self.services.models_manager,
            self.services
                .network_proxy
                .as_ref()
                .and_then(|started_proxy| {
                    Self::managed_network_proxy_active_for_permission_profile(
                        &session_configuration.permission_profile(),
                    )
                    .then(|| started_proxy.proxy())
                }),
            environment,
            turn_environments,
            cwd,
            sub_id,
            skills_outcome,
            goal_tools_supported,
        );
        turn_context.realtime_active = self.conversation.running_state().await.is_some();

        if let Some(final_schema) = final_output_json_schema {
            turn_context.final_output_json_schema = final_schema;
        }
        let turn_context = Arc::new(turn_context);
        turn_context.turn_metadata_state.spawn_git_enrichment_task();
        turn_context
    }

    pub(crate) async fn maybe_emit_unknown_model_warning_for_turn(&self, tc: &TurnContext) {
        // Skip warning for custom providers - their models are not in the bundled catalog
        // and using fallback metadata is expected and reasonable.
        let is_builtin_provider =
            BUILT_IN_PROVIDERS.contains(&tc.config.model_provider_id.as_str());

        if tc.model_info.used_fallback_model_metadata && is_builtin_provider {
            self.send_event(
                tc,
                EventMsg::Warning(WarningEvent {
                    message: format!(
                        "Model metadata for `{}` not found. Defaulting to fallback metadata; this can degrade performance and cause issues.",
                        tc.model_info.slug
                    ),
                }),
            )
            .await;
        }
    }

    pub(crate) async fn new_default_turn(&self) -> Arc<TurnContext> {
        self.new_default_turn_with_sub_id(self.next_internal_sub_id())
            .await
    }

    pub(crate) async fn new_default_turn_with_sub_id(&self, sub_id: String) -> Arc<TurnContext> {
        let session_configuration = {
            let state = self.state.lock().await;
            state.session_configuration.clone()
        };
        let turn_environments =
            match self.resolve_turn_environments(&session_configuration.environments) {
                Ok(turn_environments) => turn_environments,
                Err(err) => {
                    warn!("failed to resolve stored session environments: {err}");
                    Vec::new()
                }
            };

        self.new_turn_from_configuration(
            sub_id,
            session_configuration,
            /*final_output_json_schema*/ None,
            turn_environments,
        )
        .await
    }
}
