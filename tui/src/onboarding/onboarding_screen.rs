//! Onboarding screen orchestration and top-level keyboard routing.
//!
//! The onboarding flow is a small state machine over visible steps:
//! - [`Step::ProviderSelect`] surfaces the welcome-screen picker with inline
//!   API key input.
//! - [`Step::TrustDirectory`] is the existing trust prompt.
//!
//! The legacy `WelcomeWidget` / `AuthModeWidget` sources are preserved but are
//! no longer wired into onboarding; the welcome experience is rendered inside
//! [`ProviderSelect`] directly.

use agere_exec_server::LOCAL_FS;
use agere_git_utils::resolve_root_git_project_for_trust;
use crossterm::event::KeyEvent;
use crossterm::event::KeyEventKind;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::prelude::Widget;
use ratatui::style::Stylize;
use ratatui::text::Span;
use ratatui::widgets::Clear;
use ratatui::widgets::WidgetRef;

use crate::legacy_core::config::Config;
use crate::onboarding::custom_provider::CustomProvider;
use crate::onboarding::custom_provider::CustomProviderResult;
use crate::onboarding::provider_select::ProviderSelect;
use crate::onboarding::provider_select::SelectResult;
use crate::onboarding::provider_toml::ProviderEntry;
use crate::onboarding::trust_directory::TrustDirectorySelection;
use crate::onboarding::trust_directory::TrustDirectoryWidget;
use crate::tui::FrameRequester;
use crate::tui::Tui;
use crate::tui::TuiEvent;
use agere_model_provider_info::BUILT_IN_PROVIDERS;
use agere_model_provider_info::WireApi;
use color_eyre::eyre::Result;

#[allow(clippy::large_enum_variant)]
enum Step {
    ProviderSelect(ProviderSelect),
    CustomProvider(CustomProvider),
    TrustDirectory(TrustDirectoryWidget),
}

pub(crate) trait KeyboardHandler {
    fn handle_key_event(&mut self, key_event: KeyEvent);
    fn handle_paste(&mut self, _pasted: String) {}
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum StepState {
    Hidden,
    InProgress,
    Complete,
}

pub(crate) trait StepStateProvider {
    fn get_step_state(&self) -> StepState;
}

pub(crate) struct OnboardingScreen {
    request_frame: FrameRequester,
    agere_home: std::path::PathBuf,
    steps: Vec<Step>,
    is_done: bool,
    should_exit: bool,
    /// Active provider id from initial config; surfaced as the focused row.
    active_provider: Option<String>,
}

pub(crate) struct OnboardingScreenArgs {
    pub show_provider_screen: bool,
    pub show_trust_screen: bool,
    pub config: Config,
}

pub(crate) struct OnboardingResult {
    pub directory_trust_decision: Option<TrustDirectorySelection>,
    pub should_exit: bool,
}

impl OnboardingScreen {
    pub(crate) async fn new(tui: &mut Tui, args: OnboardingScreenArgs) -> Self {
        let OnboardingScreenArgs {
            show_provider_screen,
            show_trust_screen,
            config,
        } = args;

        let cwd = config.cwd.to_path_buf();
        let agere_home = config.agere_home.to_path_buf();
        let mut steps: Vec<Step> = Vec::new();

        let archive = crate::onboarding::provider_toml::load(&agere_home);
        let builtins = crate::onboarding::provider_templates::builtin_templates();
        let active_provider = if config.model_provider_id.is_empty() {
            None
        } else {
            Some(config.model_provider_id.clone())
        };

        let _name_conflict_set: std::collections::HashSet<String> =
            archive.providers.iter().map(|p| p.name.clone()).collect();
        if show_provider_screen {
            let mut select = ProviderSelect::new(
                archive,
                agere_home.clone(),
                builtins,
                active_provider.clone(),
            );
            select.enable_welcome_banner();
            // The fetch is kicked off by `run_onboarding_app`; show the spinner
            // hint until it resolves.
            select.set_remote_state(
                crate::onboarding::provider_templates::TemplateLoadState::Loading,
            );
            steps.push(Step::ProviderSelect(select));
        }

        #[cfg(target_os = "windows")]
        let show_windows_access_restriction_hint = config.permissions.windows_access_mode.is_none();
        #[cfg(not(target_os = "windows"))]
        let show_windows_access_restriction_hint = false;

        if show_trust_screen {
            let trust_target = resolve_root_git_project_for_trust(LOCAL_FS.as_ref(), &config.cwd)
                .await
                .map(Into::into)
                .unwrap_or_else(|| cwd.clone());
            steps.push(Step::TrustDirectory(TrustDirectoryWidget {
                cwd,
                trust_target,
                agere_home: agere_home.clone(),
                show_windows_access_restriction_hint,
                should_quit: false,
                selection: None,
                highlighted: TrustDirectorySelection::Trust,
                error: None,
            }));
        }

        Self {
            request_frame: tui.frame_requester(),
            agere_home,
            steps,
            is_done: false,
            should_exit: false,
            active_provider,
        }
    }

    fn current_active_step_idx(&self) -> Option<usize> {
        // Steps are ordered: the first InProgress step takes focus.
        // When a step completes, the next one becomes active.
        self.steps.iter().enumerate().find_map(|(idx, step)| {
            matches!(step.get_step_state(), StepState::InProgress).then_some(idx)
        })
    }

    pub(crate) fn is_done(&self) -> bool {
        self.is_done || self.current_active_step_idx().is_none()
    }

    pub fn directory_trust_decision(&self) -> Option<TrustDirectorySelection> {
        self.steps
            .iter()
            .find_map(|step| {
                if let Step::TrustDirectory(TrustDirectoryWidget { selection, .. }) = step {
                    Some(*selection)
                } else {
                    None
                }
            })
            .flatten()
    }

    pub fn should_exit(&self) -> bool {
        self.should_exit
    }

    pub(crate) fn update_remote_templates(
        &mut self,
        state: crate::onboarding::provider_templates::TemplateLoadState,
    ) {
        if let Some(Step::ProviderSelect(select)) = self
            .steps
            .iter_mut()
            .find(|s| matches!(s, Step::ProviderSelect(_)))
        {
            select.set_remote_state(state);
        }
    }

    /// Pull any user-actionable result out of the current ProviderSelect step
    /// and advance the state machine accordingly.
    fn process_pending_results(&mut self) {
        // First, extract any result from the ProviderSelect step.
        let result = if let Some(Step::ProviderSelect(select)) =
            self.find_step_mut(|s| matches!(s, Step::ProviderSelect(_)))
        {
            select.take_result()
        } else {
            None
        };

        // Then process the result without holding a mutable borrow on self.steps.
        match result {
            Some(SelectResult::Cancel) => {
                // Welcome screen: ESC cannot skip provider selection.
                // Even if a Cancel somehow arrives, ignore it when no provider
                // has been configured yet.
                if self.active_provider.is_none() {
                    // no-op — force explicit provider choice
                } else {
                    // Provider exists: mark picker done and move to trust step.
                    if let Some(Step::ProviderSelect(select)) =
                        self.find_step_mut(|s| matches!(s, Step::ProviderSelect(_)))
                    {
                        select.mark_done();
                    }
                }
            }
            Some(SelectResult::ProceedToNext { name }) => {
                // Provider with key confirmed: switch to config.toml and proceed to next step.
                if let Err(err) = self.apply_provider_switch(&name) {
                    tracing::warn!(error = %err, "failed to switch provider");
                }
                self.active_provider = Some(name);
                if let Some(Step::ProviderSelect(select)) =
                    self.find_step_mut(|s| matches!(s, Step::ProviderSelect(_)))
                {
                    select.mark_done();
                }
            }
            Some(SelectResult::SaveKey { name, api_key }) => {
                // API key saved: persist to provider.toml only (no switch to config.toml).
                match self.persist_inline_key(&name, &api_key) {
                    Ok(()) => {
                        // Mark as active so the next Enter proceeds and the
                        // rebuilt picker snaps focus to it.
                        self.active_provider = Some(name.clone());
                        // Rebuild picker to show updated state (provider now has key, shown as CompleteSaved).
                        self.rebuild_select_step();
                    }
                    Err(err) => {
                        tracing::warn!(error = %err, "failed to persist inline API key for '{name}'");
                    }
                }
            }
            Some(SelectResult::DeleteRequested(name)) => {
                let _ = crate::onboarding::provider_toml::remove(&self.agere_home, &name);
                if self.active_provider.as_deref() == Some(&name) {
                    self.active_provider = None;
                }
                self.rebuild_select_step();
            }
            Some(SelectResult::OpenCustomProvider) => {
                // Switch to the custom provider configuration page.
                let existing_names = self.collect_existing_names();
                let cp = CustomProvider::new(existing_names);
                self.steps.insert(0, Step::CustomProvider(cp));
            }
            None => {}
        }

        // Process CustomProvider results.
        let cp_result = if let Some(Step::CustomProvider(cp)) =
            self.find_step_mut(|s| matches!(s, Step::CustomProvider(_)))
        {
            cp.take_result()
        } else {
            None
        };

        match cp_result {
            Some(CustomProviderResult::Back) => {
                self.steps.retain(|s| !matches!(s, Step::CustomProvider(_)));
            }
            Some(CustomProviderResult::Saved {
                name,
                base_url,
                wire_api,
                api_key,
                models,
            }) => {
                // Persist to provider.toml.
                let encrypted_api_key = crate::crypto::encrypt_api_key(&api_key, &self.agere_home)
                    .map_err(std::io::Error::other);
                match encrypted_api_key {
                    Ok(encrypted) => {
                        let entry = ProviderEntry {
                            name,
                            base_url,
                            wire_api,
                            env_key: None,
                            encrypted_api_key: Some(encrypted),
                            is_custom: true,
                            models,
                        };
                        if let Err(err) =
                            crate::onboarding::provider_toml::upsert(&self.agere_home, &entry)
                        {
                            tracing::warn!(error = %err, "failed to persist custom provider");
                        }
                        // Remove CustomProvider step and rebuild ProviderSelect.
                        self.steps.retain(|s| !matches!(s, Step::CustomProvider(_)));
                        self.rebuild_select_step();
                    }
                    Err(err) => {
                        tracing::warn!(error = %err, "failed to encrypt custom provider API key");
                        // Keep the CustomProvider step open so the user can retry.
                    }
                }
            }
            None => {}
        }
    }

    /// Persist the API key entered inline to provider.toml (no switch to config.toml).
    fn persist_inline_key(&self, name: &str, api_key: &str) -> std::io::Result<()> {
        // Find the template or existing entry for this provider.
        let archive = crate::onboarding::provider_toml::load(&self.agere_home);
        let entry = if let Some(existing) = archive.find(name) {
            // Update existing entry with the new API key.
            let encrypted_api_key = crate::crypto::encrypt_api_key(api_key, &self.agere_home)
                .map_err(std::io::Error::other)?;
            ProviderEntry {
                encrypted_api_key: Some(encrypted_api_key),
                ..existing.clone()
            }
        } else {
            // Find the template — search builtins first, then remote templates
            // that have already been loaded.  Remote-only providers (e.g. regional
            // providers not in the embedded JSON) must also be persistable.
            let template = self.find_provider_template(name).cloned().ok_or_else(|| {
                std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    format!("provider '{name}' not found in archive or templates"),
                )
            })?;
            let encrypted_api_key = crate::crypto::encrypt_api_key(api_key, &self.agere_home)
                .map_err(std::io::Error::other)?;
            ProviderEntry {
                name: template.name.clone(),
                base_url: template.base_url.clone(),
                wire_api: template.wire_api,
                env_key: template.env_key.clone(),
                encrypted_api_key: Some(encrypted_api_key),
                is_custom: false,
                models: template.models,
            }
        };

        // Persist to provider.toml only — no switch to config.toml here.
        // Switching happens only when user explicitly presses Enter on a provider to proceed.
        crate::onboarding::provider_toml::upsert(&self.agere_home, &entry)?;

        Ok(())
    }

    /// Look up a provider template by name across builtins and already-loaded
    /// remote templates.  Used by `persist_inline_key` so that remote-only
    /// providers can be saved even before the fetch completes.
    fn find_provider_template(
        &self,
        name: &str,
    ) -> Option<&crate::onboarding::provider_templates::ProviderTemplate> {
        for s in &self.steps {
            if let Step::ProviderSelect(select) = s
                && let Some(t) = select.find_template(name)
            {
                return Some(t);
            }
        }
        None
    }

    fn find_step_mut<F>(&mut self, predicate: F) -> Option<&mut Step>
    where
        F: Fn(&Step) -> bool,
    {
        self.steps.iter_mut().find(|step| predicate(step))
    }

    fn rebuild_select_step(&mut self) {
        // Preserve the current remote_state so remote templates aren't lost
        // when the picker is rebuilt after saving a key.
        let preserved_remote = self
            .steps
            .iter_mut()
            .find_map(|s| {
                if let Step::ProviderSelect(s) = s {
                    Some(s.take_remote_state())
                } else {
                    None
                }
            })
            .unwrap_or(crate::onboarding::provider_templates::TemplateLoadState::Loading);

        let archive = crate::onboarding::provider_toml::load(&self.agere_home);
        let builtins = crate::onboarding::provider_templates::builtin_templates();
        let mut new_select = ProviderSelect::new(
            archive,
            self.agere_home.clone(),
            builtins,
            self.active_provider.clone(),
        );
        new_select.set_remote_state(preserved_remote);

        if let Some(slot) = self
            .steps
            .iter_mut()
            .find(|s| matches!(s, Step::ProviderSelect(_)))
        {
            *slot = Step::ProviderSelect(new_select);
        } else {
            self.steps.insert(0, Step::ProviderSelect(new_select));
        }
    }

    fn collect_existing_names(&self) -> Vec<String> {
        let archive = crate::onboarding::provider_toml::load(&self.agere_home);
        archive.providers.iter().map(|p| p.name.clone()).collect()
    }

    fn apply_provider_switch(&self, name: &str) -> std::io::Result<()> {
        let archive = crate::onboarding::provider_toml::load(&self.agere_home);
        let Some(entry) = archive.find(name) else {
            return Err(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!("provider '{name}' not present in provider.toml"),
            ));
        };
        let Some(model) = entry.models.first() else {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("provider '{name}' has no models configured"),
            ));
        };
        apply_provider_switch_blocking(
            &self.agere_home,
            name,
            &model.name,
            entry.get_api_key(&self.agere_home).as_str(),
            entry.base_url.as_str(),
            entry.wire_api,
            entry.env_key.as_deref(),
            entry.models.clone(),
            entry.is_custom,
        )
    }
}

fn apply_provider_switch_blocking(
    agere_home: &std::path::Path,
    name: &str,
    default_model: &str,
    api_key: &str,
    base_url: &str,
    wire_api: WireApi,
    _env_key: Option<&str>,
    models: Vec<agere_config::config_toml::ModelConfig>,
    is_custom: bool,
) -> std::io::Result<()> {
    use crate::legacy_core::config::edit::ConfigEdit;
    use crate::legacy_core::config::edit::ConfigEditsBuilder;

    let mut builder = ConfigEditsBuilder::new(agere_home).with_edits(vec![
        ConfigEdit::SetPath {
            segments: vec!["model_provider".to_string()],
            value: toml_edit::value(name),
        },
        ConfigEdit::SetPath {
            segments: vec!["model".to_string()],
            value: toml_edit::value(default_model),
        },
        ConfigEdit::SetPath {
            segments: vec![
                "model_providers".to_string(),
                name.to_string(),
                "experimental_bearer_token".to_string(),
            ],
            value: toml_edit::value(api_key),
        },
    ]);

    if is_custom || !BUILT_IN_PROVIDERS.contains(&name) {
        builder = builder.with_edits(vec![
            ConfigEdit::SetPath {
                segments: vec![
                    "model_providers".to_string(),
                    name.to_string(),
                    "base_url".to_string(),
                ],
                value: toml_edit::value(base_url),
            },
            ConfigEdit::SetPath {
                segments: vec![
                    "model_providers".to_string(),
                    name.to_string(),
                    "wire_api".to_string(),
                ],
                value: toml_edit::value(wire_api.to_string()),
            },
        ]);
        // Note: env_key is NOT written to config.toml to avoid "Missing environment variable" errors.
        // The experimental_bearer_token is sufficient for authentication.
    }

    // Write model_context_window from the first model if available.
    if let Some(ctx) = models.first().and_then(|m| m.context_window) {
        builder = builder.with_edits(vec![ConfigEdit::SetPath {
            segments: vec!["model_context_window".to_string()],
            value: toml_edit::value(ctx),
        }]);
    }

    // Write default reasoning effort based on the provider's wire_api.
    builder = builder.with_edits(vec![ConfigEdit::SetPath {
        segments: vec!["model_reasoning_effort".to_string()],
        value: toml_edit::value(
            crate::model_preset_builder::default_reasoning_effort(wire_api).to_string(),
        ),
    }]);

    builder = builder.replace_models_table(models);
    builder
        .apply_blocking()
        .map_err(|e| std::io::Error::other(format!("{e}")))
}

impl KeyboardHandler for OnboardingScreen {
    fn handle_key_event(&mut self, key_event: KeyEvent) {
        if !matches!(key_event.kind, KeyEventKind::Press | KeyEventKind::Repeat) {
            return;
        }
        // Global quit: only Ctrl-C / Ctrl-D escape onboarding without finishing.
        // Plain 'q' is reserved as a filter character on the picker.
        let is_ctrl_quit = key_event
            .modifiers
            .contains(crossterm::event::KeyModifiers::CONTROL)
            && matches!(
                key_event.code,
                crossterm::event::KeyCode::Char('c') | crossterm::event::KeyCode::Char('d')
            );
        if is_ctrl_quit {
            self.should_exit = true;
            self.is_done = true;
            return;
        }

        if let Some(idx) = self.current_active_step_idx() {
            self.steps[idx].handle_key_event(key_event);
        }

        self.process_pending_results();

        if self.steps.iter().any(|step| {
            if let Step::TrustDirectory(widget) = step {
                widget.should_quit()
            } else {
                false
            }
        }) {
            self.should_exit = true;
            self.is_done = true;
        }
        self.request_frame.schedule_frame();
    }

    fn handle_paste(&mut self, pasted: String) {
        if pasted.is_empty() {
            return;
        }
        if let Some(idx) = self.current_active_step_idx() {
            self.steps[idx].handle_paste(pasted);
        }
        self.process_pending_results();
        self.request_frame.schedule_frame();
    }
}

impl WidgetRef for &OnboardingScreen {
    fn render_ref(&self, area: Rect, buf: &mut Buffer) {
        Clear.render(area, buf);
        if let Some(idx) = self.current_active_step_idx() {
            let (content_area, footer_area) =
                crate::onboarding::onboarding_footer::split_onboarding_area(area);
            self.steps[idx].render_ref(content_area, buf);

            let hints = self.steps[idx].footer_hints();
            let footer = crate::onboarding::onboarding_footer::OnboardingFooter::new(hints);
            footer.render_ref(footer_area, buf);
        }
    }
}

impl OnboardingScreen {
    fn cursor_pos(&self, area: Rect) -> Option<(u16, u16)> {
        let idx = self.current_active_step_idx()?;
        let (content_area, _) = crate::onboarding::onboarding_footer::split_onboarding_area(area);
        self.steps[idx].cursor_pos(content_area)
    }
}

impl KeyboardHandler for Step {
    fn handle_key_event(&mut self, key_event: KeyEvent) {
        match self {
            Step::ProviderSelect(widget) => widget.handle_key_event(key_event),
            Step::CustomProvider(widget) => widget.handle_key_event(key_event),
            Step::TrustDirectory(widget) => widget.handle_key_event(key_event),
        }
    }

    fn handle_paste(&mut self, pasted: String) {
        match self {
            Step::ProviderSelect(widget) => widget.handle_paste(pasted),
            Step::CustomProvider(widget) => widget.handle_paste(pasted),
            Step::TrustDirectory(widget) => widget.handle_paste(pasted),
        }
    }
}

impl StepStateProvider for Step {
    fn get_step_state(&self) -> StepState {
        match self {
            Step::ProviderSelect(w) => w.get_step_state(),
            Step::CustomProvider(w) => w.get_step_state(),
            Step::TrustDirectory(w) => w.get_step_state(),
        }
    }
}

impl Step {
    /// Return the key hint spans for the onboarding footer.
    fn footer_hints(&self) -> Vec<Span<'_>> {
        match self {
            Step::ProviderSelect(w) => w.footer_hints(),
            Step::CustomProvider(w) => w.footer_hints(),
            Step::TrustDirectory(_) => vec![
                "  ".into(),
                "↑↓".bold(),
                " navigate  ".dim(),
                "⏎".bold(),
                " confirm".dim(),
            ],
        }
    }

    fn cursor_pos(&self, area: Rect) -> Option<(u16, u16)> {
        match self {
            Step::ProviderSelect(widget) => widget.cursor_pos(area),
            Step::TrustDirectory(_) => None,
            Step::CustomProvider(widget) => widget.cursor_pos(area),
        }
    }
}

impl WidgetRef for Step {
    fn render_ref(&self, area: Rect, buf: &mut Buffer) {
        match self {
            Step::ProviderSelect(widget) => widget.render_ref(area, buf),
            Step::CustomProvider(widget) => widget.render_ref(area, buf),
            Step::TrustDirectory(widget) => widget.render_ref(area, buf),
        }
    }
}

pub(crate) async fn run_onboarding_app(
    args: OnboardingScreenArgs,
    _app_server: Option<&mut crate::app_server_session::AppServerSession>,
    tui: &mut Tui,
) -> Result<OnboardingResult> {
    use tokio_stream::StreamExt;

    let alt = AltScreenGuard::enter(tui);
    let mut onboarding_screen = OnboardingScreen::new(alt.tui, args).await;

    // If the global cache already has templates (from startup background
    // fetch), use them directly — no flicker.
    if let Some(cached) = crate::onboarding::provider_templates::get_cached_templates() {
        onboarding_screen.update_remote_templates(
            crate::onboarding::provider_templates::TemplateLoadState::Loaded(cached),
        );
    }

    // Otherwise kick off a one-shot fetch. When done, the result is written
    // into both the onboarding screen AND the global cache so that the
    // subsequent /provider wizard sees it without re-fetching.
    let fetch_task = tokio::spawn(async {
        crate::onboarding::provider_templates::fetch_remote_templates().await
    });

    alt.tui.draw(u16::MAX, |frame| {
        frame.render_widget_ref(&onboarding_screen, frame.area());
        if let Some((x, y)) = onboarding_screen.cursor_pos(frame.area()) {
            frame.set_cursor_position((x, y));
        }
    })?;

    let tui_events = alt.tui.event_stream();
    tokio::pin!(tui_events);
    tokio::pin!(fetch_task);
    let mut fetch_done = false;
    while !onboarding_screen.is_done() {
        tokio::select! {
            biased;
            // Background template fetch completed — splice the result into the
            // picker and rerender. After this fires once we drop the future.
            res = &mut fetch_task, if !fetch_done => {
                fetch_done = true;
                let state = match res {
                    Ok(Ok(templates)) if !templates.is_empty() => {
                        crate::onboarding::provider_templates::TemplateLoadState::Loaded(templates)
                    }
                    // Empty remote list → treat like a failure and stick with
                    // the offline JSON so the picker is never blank.
                    Ok(Ok(_empty)) => crate::onboarding::provider_templates::TemplateLoadState::Failed(
                        "no providers in upstream list".to_string(),
                    ),
                    Ok(Err(msg)) => {
                        crate::onboarding::provider_templates::TemplateLoadState::Failed(msg)
                    }
                    Err(join_err) => {
                        crate::onboarding::provider_templates::TemplateLoadState::Failed(
                            join_err.to_string(),
                        )
                    }
                };
                // Also update the global cache so /provider wizard sees it.
                crate::onboarding::provider_templates::update_cache(state.clone());
                onboarding_screen.update_remote_templates(state);
                let _ = alt.tui.draw(u16::MAX, |frame| {
                    frame.render_widget_ref(&onboarding_screen, frame.area());
                    if let Some((x, y)) = onboarding_screen.cursor_pos(frame.area()) {
                        frame.set_cursor_position((x, y));
                    }
                });
            }
            event = tui_events.next() => {
                let Some(event) = event else { break };
                match event {
                    TuiEvent::Key(key_event) => {
                        onboarding_screen.handle_key_event(key_event);
                        // Trigger render after key event to show inline input changes.
                        let _ = alt.tui.draw(u16::MAX, |frame| {
                            frame.render_widget_ref(&onboarding_screen, frame.area());
                            if let Some((x, y)) = onboarding_screen.cursor_pos(frame.area()) {
                                frame.set_cursor_position((x, y));
                            }
                        });
                    }
                    TuiEvent::Paste(text) => {
                        onboarding_screen.handle_paste(text);
                        let _ = alt.tui.draw(u16::MAX, |frame| {
                            frame.render_widget_ref(&onboarding_screen, frame.area());
                            if let Some((x, y)) = onboarding_screen.cursor_pos(frame.area()) {
                                frame.set_cursor_position((x, y));
                            }
                        });
                    }
                    TuiEvent::Draw | TuiEvent::Resize => {
                        let _ = alt.tui.draw(u16::MAX, |frame| {
                            frame.render_widget_ref(&onboarding_screen, frame.area());
                            if let Some((x, y)) = onboarding_screen.cursor_pos(frame.area()) {
                                frame.set_cursor_position((x, y));
                            }
                        });
                    }
                }
            }
        }
    }
    Ok(OnboardingResult {
        directory_trust_decision: onboarding_screen.directory_trust_decision(),
        should_exit: onboarding_screen.should_exit(),
    })
}

struct AltScreenGuard<'a> {
    tui: &'a mut Tui,
}

impl<'a> AltScreenGuard<'a> {
    fn enter(tui: &'a mut Tui) -> Self {
        let _ = tui.enter_alt_screen();
        Self { tui }
    }
}

impl Drop for AltScreenGuard<'_> {
    fn drop(&mut self) {
        let _ = self.tui.leave_alt_screen();
    }
}
