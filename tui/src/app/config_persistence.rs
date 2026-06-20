//! Runtime configuration persistence helpers for the TUI app.
//!
//! This module owns the app-level glue between config.toml edits, in-memory `Config` refreshes,
//! and the ChatWidget copy of session settings, keeping persistence-heavy code out of the main app
//! loop.

use super::*;
use crate::onboarding::provider_toml::ProviderEntry;
use agere_model_provider_info::BUILT_IN_PROVIDERS;
use std::path::Path;

impl App {
    pub(super) async fn rebuild_config_for_cwd(&self, cwd: PathBuf) -> Result<Config> {
        let mut overrides = self.harness_overrides.clone();
        overrides.cwd = Some(cwd.clone());
        let cwd_display = cwd.display().to_string();
        ConfigBuilder::default()
            .agere_home(self.config.agere_home.to_path_buf())
            .cli_overrides(self.cli_kv_overrides.clone())
            .harness_overrides(overrides)
            .build()
            .await
            .wrap_err_with(|| format!("Failed to rebuild config for cwd {cwd_display}"))
    }

    pub(super) async fn refresh_in_memory_config_from_disk(&mut self) -> Result<()> {
        let mut config = self
            .rebuild_config_for_cwd(self.chat_widget.config_ref().cwd.to_path_buf())
            .await?;
        self.apply_runtime_policy_overrides(&mut config);
        self.config = config;
        self.chat_widget.sync_plugin_mentions_config(&self.config);
        self.chat_widget.sync_provider_config(&self.config);
        Ok(())
    }

    pub(super) async fn refresh_in_memory_config_from_disk_best_effort(&mut self, action: &str) {
        if let Err(err) = self.refresh_in_memory_config_from_disk().await {
            tracing::warn!(
                error = %err,
                action,
                "failed to refresh config before thread transition; continuing with current in-memory config"
            );
        }
    }

    /// Apply a `/provider <name>` switch: writes mirrored fields into
    /// `config.toml`, reloads the app-server config, and surfaces a short
    /// history note when a conversation is already running.
    pub(super) async fn handle_switch_provider(
        &mut self,
        app_server: &mut crate::app_server_session::AppServerSession,
        name: String,
    ) {
        let archive = crate::onboarding::provider_toml::load(&self.config.agere_home);
        let Some(entry) = archive.find(&name).cloned() else {
            self.chat_widget
                .add_error_message(format!("Provider '{name}' is not configured."));
            return;
        };
        let Some(default_model) = entry.models.first().map(|m| m.name.clone()) else {
            self.chat_widget
                .add_error_message(format!("Provider '{name}' has no models configured."));
            return;
        };
        // Resolve API key: first try encrypted_api_key (decrypted), then env_key environment variable.
        // Also determine what to write to config.toml:
        // - If encrypted_api_key exists (decrypted): write to experimental_bearer_token, don't write env_key
        // - If only env_key exists: write env_key, let runtime resolve from environment
        let (api_key, use_env_key) = resolve_api_key(&entry, &self.config.agere_home);
        if api_key.is_empty() {
            let env_hint = entry
                .env_key
                .as_deref()
                .map(|e| format!(" (or set env var {e})"))
                .unwrap_or_default();
            self.chat_widget.add_error_message(format!(
                "Provider '{name}' is missing an API key{env_hint}."
            ));
            return;
        }

        let default_model_context_window = entry.models.first().and_then(|m| m.context_window);
        let mut edits = vec![
            ConfigEdit::SetPath {
                segments: vec!["model_provider".to_string()],
                value: toml_edit::value(name.clone()),
            },
            ConfigEdit::SetModel {
                model: Some(default_model.clone()),
                effort: None,
                context_window: default_model_context_window,
            },
        ];
        edits.push(ConfigEdit::SetPath {
            segments: vec!["model_reasoning_effort".to_string()],
            value: toml_edit::value(
                crate::model_preset_builder::default_reasoning_effort(entry.wire_api).to_string(),
            ),
        });
        // Only write experimental_bearer_token if we have a direct api_key (not from env).
        if !use_env_key {
            edits.push(ConfigEdit::SetPath {
                segments: vec![
                    "model_providers".to_string(),
                    name.clone(),
                    "experimental_bearer_token".to_string(),
                ],
                value: toml_edit::value(api_key),
            });
        }
        let mut builder = ConfigEditsBuilder::new(&self.config.agere_home).with_edits(edits);
        if !BUILT_IN_PROVIDERS.contains(&name.as_str()) {
            builder = builder.with_edits(vec![
                ConfigEdit::SetPath {
                    segments: vec![
                        "model_providers".to_string(),
                        name.clone(),
                        "base_url".to_string(),
                    ],
                    value: toml_edit::value(entry.base_url.clone()),
                },
                ConfigEdit::SetPath {
                    segments: vec![
                        "model_providers".to_string(),
                        name.clone(),
                        "wire_api".to_string(),
                    ],
                    value: toml_edit::value(entry.wire_api.to_string()),
                },
            ]);
            // Only write env_key if we're using environment variable for API key.
            if use_env_key && entry.env_key.is_some() {
                builder = builder.with_edits(vec![ConfigEdit::SetPath {
                    segments: vec![
                        "model_providers".to_string(),
                        name.clone(),
                        "env_key".to_string(),
                    ],
                    value: toml_edit::value(entry.env_key.clone().unwrap()),
                }]);
            }
        }
        builder = builder.replace_models_table(entry.models.clone());

        if let Err(err) = builder.apply().await {
            self.chat_widget
                .add_error_message(format!("Failed to persist provider switch: {err}"));
            return;
        }

        if let Err(err) = app_server.reload_user_config().await {
            self.chat_widget
                .add_error_message(format!("Failed to reload config after switch: {err}"));
            return;
        }
        if let Err(err) = self.refresh_in_memory_config_from_disk().await {
            self.chat_widget
                .add_error_message(format!("Switch applied, but reload failed: {err}"));
            return;
        }
        self.chat_widget.set_session_provider(&name);
        // Update the collaboration mode's model to the default model of the new provider.
        self.chat_widget.set_model(&default_model);
        // Update reasoning effort to the default for the new provider's wire_api.
        let default_effort = crate::model_preset_builder::default_reasoning_effort(entry.wire_api);
        self.on_update_reasoning_effort(Some(default_effort));

        // Build model_catalog from provider's models list (from provider.toml).
        use crate::model_catalog::ModelCatalog;
        use agere_models_manager::collaboration_mode_presets::CollaborationModesConfig;

        let presets = crate::model_preset_builder::build_model_presets(
            entry.wire_api,
            &entry.models,
            &default_model,
        );
        let new_catalog = Arc::new(ModelCatalog::new(
            presets,
            CollaborationModesConfig {
                default_mode_request_user_input: self
                    .config
                    .features
                    .enabled(Feature::DefaultModeRequestUserInput),
            },
        ));
        self.model_catalog = new_catalog.clone();
        self.chat_widget.set_model_catalog(new_catalog);

        self.stage_provider_switch_for_displayed_thread(name.clone());
        if app_server.provider_updates_apply_locally()
            && let Some(thread_id) = self.current_displayed_thread_id()
        {
            match app_server
                .thread_provider_update(thread_id, name.clone())
                .await
            {
                Ok(_) => self.clear_pending_provider_switch_for_displayed_thread(&name),
                Err(err) => {
                    tracing::warn!(
                        error = %err,
                        provider = %name,
                        "failed to synchronize provider switch to loaded thread; next turn will carry provider override"
                    );
                }
            }
        }
        self.chat_widget
            .add_info_message(format!("Switched provider to '{name}'."), None);
    }

    pub(super) async fn rebuild_config_for_resume_or_fallback(
        &mut self,
        current_cwd: &Path,
        resume_cwd: PathBuf,
    ) -> Result<Config> {
        match self.rebuild_config_for_cwd(resume_cwd.clone()).await {
            Ok(config) => Ok(config),
            Err(err) => {
                if crate::cwds_differ(current_cwd, &resume_cwd) {
                    Err(err)
                } else {
                    let resume_cwd_display = resume_cwd.display().to_string();
                    tracing::warn!(
                        error = %err,
                        cwd = %resume_cwd_display,
                        "failed to rebuild config for same-cwd resume; using current in-memory config"
                    );
                    Ok(self.config.clone())
                }
            }
        }
    }

    pub(super) fn apply_runtime_policy_overrides(&mut self, config: &mut Config) {
        if let Some(policy) = self.runtime_approval_policy_override.as_ref()
            && let Err(err) = config.permissions.approval_policy.set(*policy)
        {
            tracing::warn!(%err, "failed to carry forward approval policy override");
            self.chat_widget.add_error_message(format!(
                "Failed to carry forward approval policy override: {err}"
            ));
        }
        if let Some(profile) = self.runtime_permission_profile_override.as_ref()
            && let Err(err) = config.permissions.set_permission_profile(profile.clone())
        {
            tracing::warn!(%err, "failed to carry forward permission profile override");
            self.chat_widget.add_error_message(format!(
                "Failed to carry forward permission profile override: {err}"
            ));
        }
    }

    pub(super) fn set_approvals_reviewer_in_app_and_widget(&mut self, reviewer: ApprovalsReviewer) {
        self.config.approvals_reviewer = reviewer;
        self.chat_widget.set_approvals_reviewer(reviewer);
    }

    pub(super) fn try_set_approval_policy_on_config(
        &mut self,
        config: &mut Config,
        policy: AskForApproval,
        user_message_prefix: &str,
        log_message: &str,
    ) -> bool {
        if let Err(err) = config.permissions.approval_policy.set(policy) {
            tracing::warn!(error = %err, "{log_message}");
            self.chat_widget
                .add_error_message(format!("{user_message_prefix}: {err}"));
            return false;
        }

        true
    }

    pub(super) fn try_set_permission_profile_on_config(
        &mut self,
        config: &mut Config,
        permission_profile: PermissionProfile,
        user_message_prefix: &str,
        log_message: &str,
    ) -> bool {
        if let Err(err) = config
            .permissions
            .set_permission_profile(permission_profile)
        {
            tracing::warn!(error = %err, "{log_message}");
            self.chat_widget
                .add_error_message(format!("{user_message_prefix}: {err}"));
            return false;
        }

        true
    }

    pub(super) async fn update_feature_flags(&mut self, updates: Vec<(Feature, bool)>) {
        if updates.is_empty() {
            return;
        }

        let auto_review_preset = auto_review_mode();
        let mut next_config = self.config.clone();
        let active_profile = self.active_profile.clone();
        let scoped_segments = |key: &str| {
            if let Some(profile) = active_profile.as_deref() {
                vec!["profiles".to_string(), profile.to_string(), key.to_string()]
            } else {
                vec![key.to_string()]
            }
        };
        let windows_access_mode_changed = false;
        let mut approval_policy_override = None;
        let mut approvals_reviewer_override = None;
        let mut permission_profile_override = None;
        let mut feature_updates_to_apply = Vec::with_capacity(updates.len());
        // Auto-Review owns `approvals_reviewer`, but disabling the feature
        // from inside a profile should not silently clear a value configured at
        // the root scope.
        let (root_approvals_reviewer_blocks_profile_disable, profile_approvals_reviewer_configured) = {
            let effective_config = next_config.config_layer_stack.effective_config();
            let root_blocks_disable = effective_config
                .as_table()
                .and_then(|table| table.get("approvals_reviewer"))
                .is_some_and(|value| value != &TomlValue::String("user".to_string()));
            let profile_configured = active_profile.as_deref().is_some_and(|profile| {
                effective_config
                    .as_table()
                    .and_then(|table| table.get("profiles"))
                    .and_then(TomlValue::as_table)
                    .and_then(|profiles| profiles.get(profile))
                    .and_then(TomlValue::as_table)
                    .is_some_and(|profile_config| profile_config.contains_key("approvals_reviewer"))
            });
            (root_blocks_disable, profile_configured)
        };
        let mut permissions_history_label: Option<&'static str> = None;
        let mut builder = ConfigEditsBuilder::new(&self.config.agere_home)
            .with_profile(self.active_profile.as_deref());

        for (feature, enabled) in updates {
            let feature_key = feature.key();
            let mut feature_edits = Vec::new();
            if feature == Feature::GuardianApproval
                && !enabled
                && self.active_profile.is_some()
                && root_approvals_reviewer_blocks_profile_disable
            {
                self.chat_widget.add_error_message(
                        "Cannot disable Auto-review in this profile because `approvals_reviewer` is configured outside the active profile.".to_string(),
                    );
                continue;
            }
            let mut feature_config = next_config.clone();
            if let Err(err) = feature_config.features.set_enabled(feature, enabled) {
                tracing::error!(
                    error = %err,
                    feature = feature_key,
                    "failed to update constrained feature flags"
                );
                self.chat_widget.add_error_message(format!(
                    "Failed to update experimental feature `{feature_key}`: {err}"
                ));
                continue;
            }
            let effective_enabled = feature_config.features.enabled(feature);
            if feature == Feature::GuardianApproval {
                let previous_approvals_reviewer = feature_config.approvals_reviewer;
                if effective_enabled {
                    // Persist the reviewer setting so future sessions keep the
                    // experiment's matching `/approvals` mode until the user
                    // changes it explicitly.
                    feature_config.approvals_reviewer = auto_review_preset.approvals_reviewer;
                    feature_edits.push(ConfigEdit::SetPath {
                        segments: scoped_segments("approvals_reviewer"),
                        value: auto_review_preset.approvals_reviewer.to_string().into(),
                    });
                    if previous_approvals_reviewer != auto_review_preset.approvals_reviewer {
                        permissions_history_label = Some("Auto-review");
                    }
                } else if !effective_enabled {
                    if profile_approvals_reviewer_configured || self.active_profile.is_none() {
                        feature_edits.push(ConfigEdit::ClearPath {
                            segments: scoped_segments("approvals_reviewer"),
                        });
                    }
                    feature_config.approvals_reviewer = ApprovalsReviewer::User;
                    if previous_approvals_reviewer != ApprovalsReviewer::User {
                        permissions_history_label = Some("Default");
                    }
                }
                approvals_reviewer_override = Some(feature_config.approvals_reviewer);
            }
            if feature == Feature::GuardianApproval && effective_enabled {
                // The feature flag alone is not enough for the live session.
                // We also align approval policy + access mode to the Auto-review
                // preset so enabling the experiment immediately
                // makes guardian review observable in the current thread.
                if !self.try_set_approval_policy_on_config(
                    &mut feature_config,
                    auto_review_preset.approval_policy,
                    "Failed to enable Auto-review",
                    "failed to set auto-review approval policy on staged config",
                ) {
                    continue;
                }
                if !self.try_set_permission_profile_on_config(
                    &mut feature_config,
                    auto_review_preset.permission_profile.clone(),
                    "Failed to enable Auto-review",
                    "failed to set auto-review permission profile on staged config",
                ) {
                    continue;
                }
                feature_edits.extend([
                    ConfigEdit::SetPath {
                        segments: scoped_segments("approval_policy"),
                        value: "on-request".into(),
                    },
                    ConfigEdit::SetPath {
                        segments: scoped_segments("access_mode"),
                        value: "workspace-write".into(),
                    },
                ]);
                approval_policy_override = Some(auto_review_preset.approval_policy);
                permission_profile_override = Some(auto_review_preset.permission_profile.clone());
            }
            next_config = feature_config;
            feature_updates_to_apply.push((feature, effective_enabled));
            builder = builder
                .with_edits(feature_edits)
                .set_feature_enabled(feature_key, effective_enabled);
        }

        // Persist first so the live session does not diverge from disk if the
        // config edit fails. Runtime/UI state is patched below only after the
        // durable config update succeeds.
        if let Err(err) = builder.apply().await {
            tracing::error!(error = %err, "failed to persist feature flags");
            self.chat_widget
                .add_error_message(format!("Failed to update experimental features: {err}"));
            return;
        }

        let memory_tool_was_enabled = self.config.features.enabled(Feature::MemoryTool);
        self.config = next_config;
        let show_memory_enable_notice =
            feature_updates_to_apply.iter().any(|(feature, enabled)| {
                *feature == Feature::MemoryTool && *enabled && !memory_tool_was_enabled
            });
        for (feature, effective_enabled) in feature_updates_to_apply {
            self.chat_widget
                .set_feature_enabled(feature, effective_enabled);
        }
        if show_memory_enable_notice {
            self.chat_widget.add_memories_enable_notice();
        }
        if approvals_reviewer_override.is_some() {
            self.set_approvals_reviewer_in_app_and_widget(self.config.approvals_reviewer);
        }
        if approval_policy_override.is_some() {
            self.chat_widget
                .set_approval_policy(self.config.permissions.approval_policy.value());
        }
        if permission_profile_override.is_some()
            && let Err(err) = self
                .chat_widget
                .set_permission_profile(self.config.permissions.permission_profile())
        {
            tracing::error!(
                error = %err,
                "failed to set auto-review permission profile on chat config"
            );
            self.chat_widget
                .add_error_message(format!("Failed to enable Auto-review: {err}"));
        }
        if permission_profile_override.is_some() {
            self.runtime_permission_profile_override =
                Some(self.config.permissions.permission_profile());
        }

        if approval_policy_override.is_some()
            || approvals_reviewer_override.is_some()
            || permission_profile_override.is_some()
        {
            self.sync_active_thread_permission_settings_to_cached_session()
                .await;
            // This uses `OverrideTurnContext` intentionally: toggling the
            // experiment should update the active thread's effective approval
            // settings immediately, just like a `/approvals` selection. Without
            // this runtime patch, the config edit would only affect future
            // sessions or turns recreated from disk.
            let op = AppCommand::override_turn_context(
                /*cwd*/ None,
                approval_policy_override,
                approvals_reviewer_override,
                permission_profile_override,
                /*windows_execution_restriction_level*/ None,
                /*model*/ None,
                /*effort*/ None,
                /*summary*/ None,
                /*service_tier*/ None,
                /*collaboration_mode*/ None,
                /*personality*/ None,
            );
            let replay_state_op =
                ThreadEventStore::op_can_change_pending_replay_state(&op).then(|| op.clone());
            let submitted = self.chat_widget.submit_op(op);
            if submitted && let Some(op) = replay_state_op.as_ref() {
                self.note_active_thread_outbound_op(op).await;
                self.refresh_pending_thread_approvals().await;
            }
        }

        if windows_access_mode_changed {
            #[cfg(target_os = "windows")]
            {
                let windows_execution_restriction_level =
                    match self.config.permissions.windows_access_mode {
                        Some(WindowsAccessModeToml::Elevated) => {
                            WindowsExecutionRestrictionLevel::Elevated
                        }
                        Some(WindowsAccessModeToml::Unelevated) => {
                            WindowsExecutionRestrictionLevel::RestrictedToken
                        }
                        None => WindowsExecutionRestrictionLevel::Disabled,
                    };
                self.app_event_tx.send(AppEvent::AgereOp(
                    AppCommand::override_turn_context(
                        /*cwd*/ None,
                        /*approval_policy*/ None,
                        /*approvals_reviewer*/ None,
                        /*permission_profile*/ None,
                        #[cfg(target_os = "windows")]
                        Some(windows_execution_restriction_level),
                        /*model*/ None,
                        /*effort*/ None,
                        /*summary*/ None,
                        /*service_tier*/ None,
                        /*collaboration_mode*/ None,
                        /*personality*/ None,
                    )
                    .into_core(),
                ));
            }
        }

        if let Some(label) = permissions_history_label {
            self.chat_widget.add_info_message(
                format!("Permissions updated to {label}"),
                /*hint*/ None,
            );
        }
    }

    pub(super) async fn update_memory_settings(
        &mut self,
        use_memories: bool,
        generate_memories: bool,
    ) -> bool {
        let active_profile = self.active_profile.clone();
        let scoped_memory_segments = |key: &str| {
            if let Some(profile) = active_profile.as_deref() {
                vec![
                    "profiles".to_string(),
                    profile.to_string(),
                    "memories".to_string(),
                    key.to_string(),
                ]
            } else {
                vec!["memories".to_string(), key.to_string()]
            }
        };
        let edits = [
            ConfigEdit::SetPath {
                segments: scoped_memory_segments("use_memories"),
                value: use_memories.into(),
            },
            ConfigEdit::SetPath {
                segments: scoped_memory_segments("generate_memories"),
                value: generate_memories.into(),
            },
        ];

        if let Err(err) = ConfigEditsBuilder::new(&self.config.agere_home)
            .with_edits(edits)
            .apply()
            .await
        {
            tracing::error!(error = %err, "failed to persist memory settings");
            self.chat_widget
                .add_error_message(format!("Failed to save memory settings: {err}"));
            return false;
        }

        self.config.memories.use_memories = use_memories;
        self.config.memories.generate_memories = generate_memories;
        self.chat_widget
            .set_memory_settings(use_memories, generate_memories);
        true
    }

    pub(super) async fn update_memory_settings_with_app_server(
        &mut self,
        app_server: &mut AppServerSession,
        use_memories: bool,
        generate_memories: bool,
    ) {
        let previous_generate_memories = self.config.memories.generate_memories;
        if !self
            .update_memory_settings(use_memories, generate_memories)
            .await
        {
            return;
        }

        if previous_generate_memories == generate_memories {
            return;
        }

        let Some(thread_id) = self.current_displayed_thread_id() else {
            return;
        };

        let mode = if generate_memories {
            ThreadMemoryMode::Enabled
        } else {
            ThreadMemoryMode::Disabled
        };

        if let Err(err) = app_server.thread_memory_mode_set(thread_id, mode).await {
            tracing::error!(error = %err, %thread_id, "failed to update thread memory mode");
            self.chat_widget.add_error_message(format!(
                "Saved memory settings, but failed to update the current thread: {err}"
            ));
        }
    }

    pub(super) async fn reset_memories_with_app_server(
        &mut self,
        app_server: &mut AppServerSession,
    ) {
        if let Err(err) = app_server.memory_reset().await {
            tracing::error!(error = %err, "failed to reset memories");
            self.chat_widget
                .add_error_message(format!("Failed to reset memories: {err}"));
            return;
        }

        self.chat_widget
            .add_info_message("Reset local memories.".to_string(), /*hint*/ None);
    }

    pub(super) fn reasoning_label(reasoning_effort: Option<ReasoningEffortConfig>) -> &'static str {
        match reasoning_effort {
            Some(ReasoningEffortConfig::Minimal) => "minimal",
            Some(ReasoningEffortConfig::Low) => "low",
            Some(ReasoningEffortConfig::Medium) => "medium",
            Some(ReasoningEffortConfig::High) => "high",
            Some(ReasoningEffortConfig::XHigh) => "xhigh",
            Some(ReasoningEffortConfig::Max) => "max",
            None | Some(ReasoningEffortConfig::None) => "default",
        }
    }

    pub(super) fn reasoning_label_for(
        model: &str,
        reasoning_effort: Option<ReasoningEffortConfig>,
    ) -> Option<&'static str> {
        (!model.starts_with("agere-auto-")).then(|| Self::reasoning_label(reasoning_effort))
    }

    pub(crate) fn token_usage(&self) -> agere_protocol::protocol::TokenUsage {
        self.chat_widget.token_usage()
    }

    pub(super) fn on_update_reasoning_effort(&mut self, effort: Option<ReasoningEffortConfig>) {
        // TODO(aibrahim): Remove this and don't use config as a state object.
        // Instead, explicitly pass the stored collaboration mode's effort into new sessions.
        self.config.model_reasoning_effort = effort;
        self.chat_widget.set_reasoning_effort(effort);
    }

    pub(super) fn on_update_personality(&mut self, personality: Personality) {
        self.config.personality = Some(personality);
        self.chat_widget.set_personality(personality);
    }

    pub(super) fn sync_tui_theme_selection(&mut self, name: String) {
        self.config.tui_theme = Some(name.clone());
        self.chat_widget.set_tui_theme(Some(name));
    }

    pub(super) fn restore_runtime_theme_from_config(&self) {
        if let Some(name) = self.config.tui_theme.as_deref()
            && let Some(theme) =
                crate::render::highlight::resolve_theme_by_name(name, Some(&self.config.agere_home))
        {
            crate::render::highlight::set_syntax_theme(theme);
            return;
        }

        let auto_theme_name = crate::render::highlight::adaptive_default_theme_name();
        if let Some(theme) = crate::render::highlight::resolve_theme_by_name(
            auto_theme_name,
            Some(&self.config.agere_home),
        ) {
            crate::render::highlight::set_syntax_theme(theme);
        }
    }

    pub(super) fn personality_label(personality: Personality) -> &'static str {
        match personality {
            Personality::None => "None",
            Personality::Friendly => "Friendly",
            Personality::Pragmatic => "Pragmatic",
        }
    }
}

/// Resolve the API key and determine whether to use env_key for config.toml.
///
/// Priority: encrypted `encrypted_api_key` (decrypted) > `env_key` environment variable.
/// Returns `(resolved_key, use_env_key)` where `use_env_key` indicates
/// whether the key came from the environment (and thus env_key should be
/// persisted in config.toml for runtime resolution).
fn resolve_api_key(entry: &ProviderEntry, agere_home: &Path) -> (String, bool) {
    // Try encrypted_api_key first (decrypts automatically via get_api_key).
    let decrypted = entry.get_api_key(agere_home);
    if !decrypted.is_empty() {
        return (decrypted, false);
    }

    // Try env_key environment variable.
    if let Some(env_key) = &entry.env_key
        && let Ok(value) = std::env::var(env_key)
        && !value.is_empty()
    {
        return (value, true);
    }

    (String::new(), false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::test_support::app_enabled_in_effective_config;
    use crate::app::test_support::make_test_app;
    use crate::test_support::PathBufExt;
    use agere_model_provider_info::WireApi;
    use agere_protocol::models::PermissionProfile;
    use agere_protocol::protocol::Event;
    use agere_protocol::protocol::EventMsg;
    use agere_protocol::protocol::SessionConfiguredEvent;
    use pretty_assertions::assert_eq;
    use tempfile::tempdir;

    #[tokio::test]
    async fn update_reasoning_effort_updates_collaboration_mode() {
        let mut app = make_test_app().await;
        app.chat_widget
            .set_reasoning_effort(Some(ReasoningEffortConfig::Medium));

        app.on_update_reasoning_effort(Some(ReasoningEffortConfig::High));

        assert_eq!(
            app.chat_widget.current_reasoning_effort(),
            Some(ReasoningEffortConfig::High)
        );
        assert_eq!(
            app.config.model_reasoning_effort,
            Some(ReasoningEffortConfig::High)
        );
    }

    #[tokio::test]
    async fn refresh_in_memory_config_from_disk_loads_latest_apps_state() -> Result<()> {
        let mut app = make_test_app().await;
        let agere_home = tempdir()?;
        app.config.agere_home = agere_home.path().to_path_buf().abs();
        let app_id = "unit_test_refresh_in_memory_config_connector".to_string();

        assert_eq!(app_enabled_in_effective_config(&app.config, &app_id), None);

        ConfigEditsBuilder::new(&app.config.agere_home)
            .with_edits([
                ConfigEdit::SetPath {
                    segments: vec!["apps".to_string(), app_id.clone(), "enabled".to_string()],
                    value: false.into(),
                },
                ConfigEdit::SetPath {
                    segments: vec![
                        "apps".to_string(),
                        app_id.clone(),
                        "disabled_reason".to_string(),
                    ],
                    value: "user".into(),
                },
            ])
            .apply()
            .await
            .expect("persist app toggle");

        assert_eq!(app_enabled_in_effective_config(&app.config, &app_id), None);

        app.refresh_in_memory_config_from_disk().await?;

        assert_eq!(
            app_enabled_in_effective_config(&app.config, &app_id),
            Some(false)
        );
        Ok(())
    }

    #[tokio::test]
    async fn refresh_in_memory_config_from_disk_best_effort_keeps_current_config_on_error()
    -> Result<()> {
        let mut app = make_test_app().await;
        let agere_home = tempdir()?;
        app.config.agere_home = agere_home.path().to_path_buf().abs();
        std::fs::write(agere_home.path().join("config.toml"), "[broken")?;
        let original_config = app.config.clone();

        app.refresh_in_memory_config_from_disk_best_effort("starting a new thread")
            .await;

        assert_eq!(app.config, original_config);
        Ok(())
    }

    #[tokio::test]
    async fn refresh_in_memory_config_from_disk_uses_active_chat_widget_cwd() -> Result<()> {
        let mut app = make_test_app().await;
        let original_cwd = app.config.cwd.clone();
        let next_cwd_tmp = tempdir()?;
        let next_cwd = next_cwd_tmp.path().to_path_buf();

        app.chat_widget.handle_agere_event(Event {
            id: String::new(),
            msg: EventMsg::SessionConfigured(SessionConfiguredEvent {
                session_id: ThreadId::new(),
                forked_from_id: None,
                thread_name: None,
                model: "gpt-test".to_string(),
                model_provider_id: "test-provider".to_string(),
                service_tier: None,
                approval_policy: AskForApproval::Never,
                approvals_reviewer: ApprovalsReviewer::User,
                permission_profile: PermissionProfile::read_only(),
                cwd: next_cwd.clone().abs(),
                reasoning_effort: None,
                history_log_id: 0,
                history_entry_count: 0,
                initial_messages: None,
                network_proxy: None,
                rollout_path: Some(PathBuf::new()),
            }),
        });

        assert_eq!(app.chat_widget.config_ref().cwd.to_path_buf(), next_cwd);
        assert_eq!(app.config.cwd, original_cwd);

        app.refresh_in_memory_config_from_disk().await?;

        assert_eq!(app.config.cwd, app.chat_widget.config_ref().cwd);
        Ok(())
    }

    #[tokio::test]
    async fn refresh_in_memory_config_from_disk_updates_resize_reflow_config() -> Result<()> {
        let mut app = make_test_app().await;
        let agere_home = tempdir()?;
        app.config.agere_home = agere_home.path().to_path_buf().abs();
        std::fs::write(
            agere_home.path().join("config.toml"),
            r#"
[tui]
terminal_resize_reflow_max_rows = 9000
"#,
        )?;

        app.refresh_in_memory_config_from_disk().await?;

        assert_eq!(
            app.config.terminal_resize_reflow.max_rows,
            crate::legacy_core::config::TerminalResizeReflowMaxRows::Limit(9000)
        );
        Ok(())
    }

    #[tokio::test]
    async fn refresh_in_memory_config_from_disk_updates_chat_widget_provider_config() -> Result<()>
    {
        let mut app = make_test_app().await;
        let agere_home = tempdir()?;
        app.config.agere_home = agere_home.path().to_path_buf().abs();

        std::fs::write(
            agere_home.path().join("config.toml"),
            r#"
model_provider = "deepseek"
model = "deepseek-chat"

[model_providers.deepseek]
name = "DeepSeek"
base_url = "https://api.deepseek.com/anthropic"
wire_api = "anthropic"
experimental_bearer_token = "sk-test"

[[models]]
name = "deepseek-chat"
context_window = 128000
"#,
        )?;

        assert_ne!(app.chat_widget.config_ref().model_provider_id, "deepseek");

        app.refresh_in_memory_config_from_disk().await?;

        assert_eq!(app.config.model_provider_id, "deepseek");
        assert_eq!(app.chat_widget.config_ref().model_provider_id, "deepseek");
        assert_eq!(
            app.chat_widget.config_ref().model_provider.wire_api,
            WireApi::Anthropic
        );
        assert_eq!(
            app.chat_widget.config_ref().model.as_deref(),
            Some("deepseek-chat")
        );
        assert_eq!(app.chat_widget.config_ref().models, app.config.models);
        Ok(())
    }

    #[tokio::test]
    async fn rebuild_config_for_resume_or_fallback_uses_current_config_on_same_cwd_error()
    -> Result<()> {
        let mut app = make_test_app().await;
        let agere_home = tempdir()?;
        app.config.agere_home = agere_home.path().to_path_buf().abs();
        std::fs::write(agere_home.path().join("config.toml"), "[broken")?;
        let current_config = app.config.clone();
        let current_cwd = current_config.cwd.clone();

        let resume_config = app
            .rebuild_config_for_resume_or_fallback(&current_cwd, current_cwd.to_path_buf())
            .await?;

        assert_eq!(resume_config, current_config);
        Ok(())
    }

    #[tokio::test]
    async fn rebuild_config_for_resume_or_fallback_errors_when_cwd_changes() -> Result<()> {
        let mut app = make_test_app().await;
        let agere_home = tempdir()?;
        app.config.agere_home = agere_home.path().to_path_buf().abs();
        std::fs::write(agere_home.path().join("config.toml"), "[broken")?;
        let current_cwd = app.config.cwd.clone();
        let next_cwd_tmp = tempdir()?;
        let next_cwd = next_cwd_tmp.path().to_path_buf();

        let result = app
            .rebuild_config_for_resume_or_fallback(&current_cwd, next_cwd)
            .await;

        assert!(result.is_err());
        Ok(())
    }

    #[tokio::test]
    async fn sync_tui_theme_selection_updates_chat_widget_config_copy() {
        let mut app = make_test_app().await;

        app.sync_tui_theme_selection("dracula".to_string());

        assert_eq!(app.config.tui_theme.as_deref(), Some("dracula"));
        assert_eq!(
            app.chat_widget.config_ref().tui_theme.as_deref(),
            Some("dracula")
        );
    }
}
