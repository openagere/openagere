//! Provider configuration wizard — shared popup surface style.
//!
//! Step 1: Provider list (compact, same style as slash command menu).
//! Step 1b: Wire API type list (same style).
//! Step 2: Field configuration.
//! Step 3: Field edit.
//!
//! Compact height is driven by actual row count. API keys are masked in display,
//! stored plaintext. Provider names are NOT capitalized — exact match with
//! provider.toml.

use agere_model_provider_info::BUILT_IN_PROVIDERS;
use agere_model_provider_info::WireApi;
use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyEventKind;
use crossterm::event::KeyModifiers;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Stylize;
use ratatui::text::Line;
use ratatui::text::Span;
use ratatui::widgets::Paragraph;
use ratatui::widgets::Wrap;

use std::cell::Cell;
use std::path::Path;

use crate::app_event::AppEvent;
use crate::app_event_sender::AppEventSender;
use crate::onboarding::provider_templates::ProviderTemplate;
use crate::onboarding::provider_toml::ProviderEntry;
use crate::onboarding::provider_toml::ProviderToml;
use crate::provider_ui;
use crate::provider_ui::ProviderFieldKind;
use crate::provider_ui::ProviderFormKind;
use crate::render::renderable::Renderable;
use unicode_width::UnicodeWidthStr;

use super::bottom_pane_view::BottomPaneView;
use super::bottom_pane_view::ViewCompletion;
use super::selection_popup_common::menu_surface_inset;
use super::selection_popup_common::render_menu_surface;

const MAX_VISIBLE_ROWS: usize = 9;

// ---------------------------------------------------------------------------
// Data carried between steps
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, Default)]
struct Data {
    name: String,
    base_url: String,
    wire_api: WireApi,
    env_key: String,
    api_key: String,
    models_str: String,
    is_custom: bool,
}

impl Data {
    fn new_custom(wire_api: WireApi, base_url: &str) -> Self {
        Self {
            name: String::new(),
            base_url: base_url.to_string(),
            wire_api,
            env_key: String::new(),
            api_key: String::new(),
            models_str: String::new(),
            is_custom: true,
        }
    }
    fn from_entry(e: &ProviderEntry, agere_home: &Path) -> Self {
        Self {
            name: e.name.clone(),
            base_url: e.base_url.clone(),
            wire_api: e.wire_api,
            env_key: e.env_key.clone().unwrap_or_default(),
            api_key: e.get_api_key(agere_home),
            models_str: provider_ui::models_to_string(&e.models),
            is_custom: e.is_custom,
        }
    }
    fn from_template(t: &ProviderTemplate) -> Self {
        Self {
            name: t.name.clone(),
            base_url: t.base_url.clone(),
            wire_api: t.wire_api,
            env_key: t.env_key.clone().unwrap_or_default(),
            api_key: String::new(),
            models_str: provider_ui::models_to_string(&t.models),
            is_custom: false,
        }
    }
    fn to_entry(
        &self,
        agere_home: &Path,
        name: String,
        base_url: String,
        api_key: String,
        models: Vec<agere_config::config_toml::ModelConfig>,
    ) -> anyhow::Result<ProviderEntry> {
        let env_key = self.env_key.trim().to_string();
        let encrypted_api_key = if api_key.is_empty() {
            None
        } else {
            Some(crate::crypto::encrypt_api_key(&api_key, agere_home)?)
        };
        Ok(ProviderEntry {
            name,
            base_url,
            wire_api: self.wire_api,
            env_key: if env_key.is_empty() {
                None
            } else {
                Some(env_key)
            },
            encrypted_api_key,
            is_custom: self.is_custom,
            models,
        })
    }
}

fn configure_title(data: &Data) -> String {
    let dn = if data.name.is_empty() {
        "New provider"
    } else {
        &data.name
    };
    let badge = if data.is_custom { "CUSTOM" } else { "REMOTE" };
    format!("◆ Configure {dn}   [{badge}]")
}

// ---------------------------------------------------------------------------
// Provider row kinds
// ---------------------------------------------------------------------------

enum PRowKind {
    SavedComplete(String),
    NeedConfig(Data),
    AddCustom,
}

struct PRow {
    name: String,
    desc: String,
    name_width: usize,
    kind: PRowKind,
}

struct ProviderListState {
    rows: Vec<PRow>,
    filtered_indices: Vec<usize>,
    sel: usize,
    scroll_offset: usize,
    query: String,
}

impl ProviderListState {
    fn new(rows: Vec<PRow>, selected_name: Option<&str>) -> Self {
        let mut state = Self {
            rows,
            filtered_indices: Vec::new(),
            sel: 0,
            scroll_offset: 0,
            query: String::new(),
        };
        state.apply_filter(selected_name);
        state
    }

    fn filtered_len(&self) -> usize {
        self.filtered_indices.len()
    }

    fn selected_row_index(&self) -> Option<usize> {
        self.filtered_indices.get(self.sel).copied()
    }

    fn selected_row(&self) -> Option<&PRow> {
        self.selected_row_index().and_then(|idx| self.rows.get(idx))
    }

    fn apply_filter(&mut self, preferred_name: Option<&str>) {
        let query = self.query.trim().to_ascii_lowercase();
        self.filtered_indices = self
            .rows
            .iter()
            .enumerate()
            .filter_map(|(idx, row)| {
                (query.is_empty() || provider_row_matches(row, &query)).then_some(idx)
            })
            .collect();

        self.sel = preferred_name
            .and_then(|name| {
                self.filtered_indices
                    .iter()
                    .position(|&idx| self.rows[idx].name == name)
            })
            .unwrap_or_else(|| self.sel.min(self.filtered_len().saturating_sub(1)));
        clamp_list_scroll_offset(
            self.sel,
            self.filtered_len(),
            MAX_VISIBLE_ROWS,
            &mut self.scroll_offset,
        );
    }
}

fn provider_row_matches(row: &PRow, query: &str) -> bool {
    row.name.to_ascii_lowercase().contains(query)
}

fn build_provider_rows(
    archive: &ProviderToml,
    builtins: &[ProviderTemplate],
    remote: Option<&[ProviderTemplate]>,
    active: &Option<String>,
    agere_home: &Path,
) -> Vec<PRow> {
    let mut rows = Vec::new();
    let mut seen = std::collections::HashSet::new();

    let push = |rows: &mut Vec<PRow>, name: &str, desc: &str, kind: PRowKind| {
        rows.push(PRow {
            name: name.to_string(),
            desc: desc.to_string(),
            name_width: name.width(),
            kind,
        });
    };

    // Add custom provider — always at the top.
    push(
        &mut rows,
        "Add custom provider",
        "create a provider from scratch",
        PRowKind::AddCustom,
    );

    // Active provider.
    if let Some(n) = active
        && let Some(e) = archive.find(n)
    {
        let hk = e.has_api_key();
        seen.insert(e.name.clone());
        push(
            &mut rows,
            &e.name,
            if hk {
                "current session default"
            } else {
                "missing API key — press Enter to configure"
            },
            if hk {
                PRowKind::SavedComplete(e.name.clone())
            } else {
                PRowKind::NeedConfig(Data::from_entry(e, agere_home))
            },
        );
    }

    // Saved providers — preserve order from provider.toml (no sorting).
    for e in &archive.providers {
        if seen.contains(&e.name) {
            continue;
        } // Skip active provider (already added).
        seen.insert(e.name.clone());
        let hk = e.has_api_key();
        if hk {
            push(
                &mut rows,
                &e.name,
                "Enter to use, Space to edit",
                PRowKind::SavedComplete(e.name.clone()),
            );
        } else {
            push(
                &mut rows,
                &e.name,
                "missing API key — press Enter to configure",
                PRowKind::NeedConfig(Data::from_entry(e, agere_home)),
            );
        }
    }

    // Templates — prefer remote if available, fallback to builtins.
    // Preserve original order from the source (no sorting).
    let template_source: Vec<_> = match remote {
        Some(r) if !r.is_empty() => r.to_vec(),
        _ => builtins.to_vec(),
    };
    for t in template_source
        .into_iter()
        .filter(|t| !BUILT_IN_PROVIDERS.contains(&t.name.as_str()))
        .filter(|t| !seen.contains(&t.name))
    {
        push(
            &mut rows,
            &t.name,
            "template — add API key to enable",
            PRowKind::NeedConfig(Data::from_template(&t)),
        );
    }

    rows
}

fn wire_api_rows() -> Vec<PRow> {
    vec![
        PRow {
            name: "OpenAI Responses".into(),
            desc: "Responses API (new)".into(),
            name_width: "OpenAI Responses".width(),
            kind: PRowKind::NeedConfig(Data::new_custom(
                WireApi::Responses,
                "https://api.openai.com/v1",
            )),
        },
        PRow {
            name: "OpenAI Chat".into(),
            desc: "Chat Completions API".into(),
            name_width: "OpenAI Chat".width(),
            kind: PRowKind::NeedConfig(Data::new_custom(
                WireApi::Chat,
                "https://api.openai.com/v1",
            )),
        },
        PRow {
            name: "Anthropic".into(),
            desc: "Messages API".into(),
            name_width: "Anthropic".width(),
            kind: PRowKind::NeedConfig(Data::new_custom(
                WireApi::Anthropic,
                "https://api.anthropic.com",
            )),
        },
    ]
}

fn clamp_list_scroll_offset(
    selected: usize,
    row_count: usize,
    max_visible: usize,
    scroll_offset: &mut usize,
) {
    if row_count == 0 || max_visible == 0 {
        *scroll_offset = 0;
        return;
    }

    let max_scroll_offset = row_count.saturating_sub(max_visible);
    *scroll_offset = (*scroll_offset).min(max_scroll_offset);

    if selected < *scroll_offset {
        *scroll_offset = selected;
    } else if selected >= *scroll_offset + max_visible {
        *scroll_offset = selected - max_visible + 1;
    }
}

fn field_form_kind(is_custom: bool) -> ProviderFormKind {
    if is_custom {
        ProviderFormKind::WizardCustom
    } else {
        ProviderFormKind::ExistingOrTemplate
    }
}

fn field_is_editable(field: ProviderFieldKind, is_custom: bool) -> bool {
    is_custom || matches!(field, ProviderFieldKind::ApiKey | ProviderFieldKind::Models)
}

fn field_display_value(field: ProviderFieldKind, data: &Data) -> String {
    match field {
        ProviderFieldKind::Name => data.name.clone(),
        ProviderFieldKind::BaseUrl => data.base_url.clone(),
        ProviderFieldKind::WireApi => provider_ui::wire_api_name(data.wire_api).to_string(),
        ProviderFieldKind::EnvKey => data.env_key.clone(),
        ProviderFieldKind::ApiKey => provider_ui::mask_key(&data.api_key),
        ProviderFieldKind::Models => data.models_str.clone(),
    }
}

fn field_raw_value(field: ProviderFieldKind, data: &Data) -> String {
    match field {
        ProviderFieldKind::Name => data.name.clone(),
        ProviderFieldKind::BaseUrl => data.base_url.clone(),
        ProviderFieldKind::WireApi => provider_ui::wire_api_name(data.wire_api).to_string(),
        ProviderFieldKind::EnvKey => data.env_key.clone(),
        ProviderFieldKind::ApiKey => data.api_key.clone(),
        ProviderFieldKind::Models => data.models_str.clone(),
    }
}

fn set_field_value(field: ProviderFieldKind, data: &mut Data, value: String) {
    match field {
        ProviderFieldKind::Name => data.name = value,
        ProviderFieldKind::BaseUrl => data.base_url = value,
        ProviderFieldKind::WireApi => {}
        ProviderFieldKind::EnvKey => data.env_key = value,
        ProviderFieldKind::ApiKey => data.api_key = value,
        ProviderFieldKind::Models => data.models_str = value,
    }
}

// ---------------------------------------------------------------------------
// Steps
// ---------------------------------------------------------------------------

enum Step {
    SelectProvider(ProviderListState),
    ConfirmDeleteProvider {
        name: String,
        sel: usize,
        scroll_offset: usize,
    },
    SelectWireApi {
        sel: usize,
    },
    ConfigureFields {
        focused_idx: usize,
    },
    EditField {
        field: ProviderFieldKind,
        buffer: String,
        cursor_pos: usize,
    },
    Done,
}

// ---------------------------------------------------------------------------
// ProviderWizard
// ---------------------------------------------------------------------------

pub(crate) struct ProviderWizard {
    archive: ProviderToml,
    builtin_templates: Vec<ProviderTemplate>,
    /// Templates fetched from remote (openagere.com). Merged into provider rows
    /// on top of builtins when available.
    remote_templates: Option<Vec<ProviderTemplate>>,
    active_provider_name: Option<String>,
    step: Step,
    config_data: Option<Data>,
    error: Option<String>,
    completion: Option<ViewCompletion>,
    app_event_tx: AppEventSender,
    agere_home: agere_utils_fs::AbsolutePathBuf,
    edit_input_width: Cell<u16>,
}

impl ProviderWizard {
    pub(crate) fn new(
        archive: ProviderToml,
        builtin_templates: Vec<ProviderTemplate>,
        active_provider_name: Option<String>,
        app_event_tx: AppEventSender,
        agere_home: agere_utils_fs::AbsolutePathBuf,
    ) -> Self {
        let rows = build_provider_rows(
            &archive,
            &builtin_templates,
            None,
            &active_provider_name,
            &agere_home,
        );
        let list = ProviderListState::new(rows, active_provider_name.as_deref());
        Self {
            archive,
            builtin_templates,
            remote_templates: None,
            active_provider_name,
            step: Step::SelectProvider(list),
            config_data: None,
            error: None,
            completion: None,
            app_event_tx,
            agere_home,
            edit_input_width: Cell::new(1),
        }
    }

    // ── Navigation ──────────────────────────────────────────────────────

    /// Called when remote template fetch completes. Refreshes provider rows
    /// if still on the SelectProvider step.
    pub(crate) fn update_remote_templates(&mut self, templates: Vec<ProviderTemplate>) {
        self.remote_templates = Some(templates);
        if let Step::SelectProvider(list) = &mut self.step {
            let selected_name = list.selected_row().map(|row| row.name.clone());
            let new_rows = build_provider_rows(
                &self.archive,
                &self.builtin_templates,
                self.remote_templates.as_deref(),
                &self.active_provider_name,
                &self.agere_home,
            );
            list.rows = new_rows;
            list.apply_filter(selected_name.as_deref());
        }
    }

    fn select_provider_row(&mut self) {
        let Step::SelectProvider(list) = &self.step else {
            return;
        };
        let Some(row) = list.selected_row() else {
            return;
        };
        match &row.kind {
            PRowKind::SavedComplete(name) => {
                self.app_event_tx
                    .send(AppEvent::SwitchProvider { name: name.clone() });
                self.completion = Some(ViewCompletion::Accepted);
                self.step = Step::Done;
            }
            PRowKind::NeedConfig(data) => {
                let d = data.clone();
                self.config_data = Some(d.clone());
                if d.is_custom {
                    self.step = Step::SelectWireApi { sel: 0 };
                } else {
                    self.step = Step::ConfigureFields { focused_idx: 0 };
                }
                self.error = None;
            }
            PRowKind::AddCustom => {
                self.config_data = Some(Data::new_custom(WireApi::Chat, ""));
                self.step = Step::SelectWireApi { sel: 0 };
                self.error = None;
            }
        }
    }

    /// Like `select_provider_row` but for editing (Space key).
    /// SavedComplete providers open for re-editing instead of switching.
    fn select_provider_row_for_edit(&mut self) {
        let Step::SelectProvider(list) = &self.step else {
            return;
        };
        let Some(row) = list.selected_row() else {
            return;
        };
        match &row.kind {
            PRowKind::SavedComplete(name) => {
                if let Some(entry) = self.archive.find(name) {
                    self.config_data = Some(Data::from_entry(entry, &self.agere_home));
                    self.step = Step::ConfigureFields { focused_idx: 0 };
                    self.error = None;
                }
            }
            PRowKind::NeedConfig(data) => {
                let d = data.clone();
                self.config_data = Some(d.clone());
                if d.is_custom {
                    self.step = Step::SelectWireApi { sel: 0 };
                } else {
                    self.step = Step::ConfigureFields { focused_idx: 0 };
                }
                self.error = None;
            }
            PRowKind::AddCustom => {
                self.config_data = Some(Data::new_custom(WireApi::Chat, ""));
                self.step = Step::SelectWireApi { sel: 0 };
                self.error = None;
            }
        }
    }

    fn delete_provider_row(&mut self) {
        let Step::SelectProvider(list) = &self.step else {
            return;
        };
        let Some(row) = list.selected_row() else {
            return;
        };
        if self.archive.find(&row.name).is_none() {
            self.error = Some("Only saved providers can be deleted".to_string());
            return;
        }
        if self.active_provider_name.as_deref() == Some(row.name.as_str()) {
            self.error =
                Some("Switch to another provider before deleting the active provider".to_string());
            return;
        }
        self.error = None;
        self.step = Step::ConfirmDeleteProvider {
            name: row.name.clone(),
            sel: list.sel,
            scroll_offset: list.scroll_offset,
        };
    }

    fn confirm_delete_provider(&mut self, name: String, previous_sel: usize, scroll_offset: usize) {
        match crate::onboarding::provider_toml::remove(&self.agere_home, &name) {
            Ok(true) => {
                self.archive = crate::onboarding::provider_toml::load(&self.agere_home);
                self.config_data = None;
                self.error = None;
                let rows = build_provider_rows(
                    &self.archive,
                    &self.builtin_templates,
                    self.remote_templates.as_deref(),
                    &self.active_provider_name,
                    &self.agere_home,
                );
                let sel = previous_sel.min(rows.len().saturating_sub(1));
                let mut list = ProviderListState::new(rows, None);
                list.sel = sel;
                list.scroll_offset = scroll_offset;
                list.apply_filter(None);
                self.step = Step::SelectProvider(list);
            }
            Ok(false) => {
                self.go_back_to_provider_list();
                self.error = Some(format!("Provider '{name}' was already deleted"));
            }
            Err(err) => {
                self.go_back_to_provider_list();
                self.error = Some(format!("failed to delete provider: {err}"));
            }
        }
    }

    fn select_wire_api_row(&mut self, idx: usize) {
        let rows = wire_api_rows();
        let Some(row) = rows.get(idx) else {
            return;
        };
        if let PRowKind::NeedConfig(data) = &row.kind {
            self.config_data = Some(data.clone());
            self.step = Step::ConfigureFields { focused_idx: 0 };
            self.error = None;
        }
    }

    /// Save data to `provider.toml` and go back to the provider list.
    /// Does NOT switch the active provider — that happens only when the
    /// user presses Enter on a provider in the list.
    fn save_and_back_to_list(&mut self) {
        let Some(data) = &self.config_data else {
            return;
        };
        let name = data.name.trim().to_string();
        let base_url = data.base_url.trim().to_string();
        let env_key = data.env_key.trim();
        let api_key = data.api_key.trim().to_string();
        let models = data.models_str.trim();

        if let Err(msg) = validate_name(&name, data, &self.archive) {
            self.error = Some(msg);
            return;
        }
        if let Err(msg) = provider_ui::validate_base_url(&base_url) {
            self.error = Some(msg);
            return;
        }
        if env_key.is_empty()
            && let Err(msg) = provider_ui::validate_api_key(&api_key)
        {
            self.error = Some(msg);
            return;
        }
        let models = match provider_ui::parse_models(models) {
            Ok(models) => models,
            Err(msg) => {
                self.error = Some(msg);
                return;
            }
        };
        let entry = match data.to_entry(&self.agere_home, name, base_url, api_key, models) {
            Ok(e) => e,
            Err(e) => {
                self.error = Some(format!("Failed to encrypt API key: {e}"));
                return;
            }
        };
        let new_name = entry.name.clone();
        if let Err(err) = crate::onboarding::provider_toml::upsert(&self.agere_home, &entry) {
            self.error = Some(format!("failed to save: {err}"));
            return;
        }
        // Refresh archive after save.
        self.archive = crate::onboarding::provider_toml::load(&self.agere_home);
        // IMPORTANT: only save to provider.toml here — do NOT switch.
        // Switching (which writes to config.toml) only happens when the
        // user explicitly presses Enter on a provider in the list.
        self.config_data = None;
        self.error = None;
        let rows = build_provider_rows(
            &self.archive,
            &self.builtin_templates,
            self.remote_templates.as_deref(),
            &self.active_provider_name,
            &self.agere_home,
        );
        // Select the newly added provider (position after "Add custom provider").
        self.step = Step::SelectProvider(ProviderListState::new(rows, Some(&new_name)));
    }

    fn go_back_to_provider_list(&mut self) {
        let selected_name = match &self.step {
            Step::SelectProvider(list) => list.selected_row().map(|row| row.name.clone()),
            _ => None,
        };
        self.config_data = None;
        self.error = None;
        let rows = build_provider_rows(
            &self.archive,
            &self.builtin_templates,
            self.remote_templates.as_deref(),
            &self.active_provider_name,
            &self.agere_home,
        );
        let preferred = selected_name
            .as_deref()
            .or(self.active_provider_name.as_deref());
        self.step = Step::SelectProvider(ProviderListState::new(rows, preferred));
    }
}

// ---------------------------------------------------------------------------
// BottomPaneView
// ---------------------------------------------------------------------------

impl BottomPaneView for ProviderWizard {
    fn view_id(&self) -> Option<&'static str> {
        Some("provider_wizard")
    }

    fn selected_index(&self) -> Option<usize> {
        if let Step::SelectProvider(list) = &self.step {
            Some(list.sel)
        } else {
            None
        }
    }

    fn as_any_mut(&mut self) -> Option<&mut dyn std::any::Any> {
        Some(self)
    }

    fn handle_key_event(&mut self, key_event: KeyEvent) {
        if !matches!(key_event.kind, KeyEventKind::Press | KeyEventKind::Repeat) {
            return;
        }
        let edit_input_width = self.edit_input_width.get().max(1);
        match &mut self.step {
            Step::SelectProvider(list) => match key_event.code {
                KeyCode::Esc => {
                    if list.query.is_empty() {
                        self.completion = Some(ViewCompletion::Cancelled);
                    } else {
                        list.query.clear();
                        list.apply_filter(self.active_provider_name.as_deref());
                    }
                }
                KeyCode::Enter => {
                    self.select_provider_row();
                }
                KeyCode::Char(' ') => {
                    self.select_provider_row_for_edit();
                }
                KeyCode::Delete => {
                    self.delete_provider_row();
                }
                KeyCode::Char('d') if key_event.modifiers.contains(KeyModifiers::CONTROL) => {
                    self.delete_provider_row();
                }
                KeyCode::Up => {
                    if list.filtered_len() > 0 && list.sel > 0 {
                        list.sel -= 1;
                        clamp_list_scroll_offset(
                            list.sel,
                            list.filtered_len(),
                            MAX_VISIBLE_ROWS,
                            &mut list.scroll_offset,
                        );
                    }
                }
                KeyCode::Down => {
                    if list.filtered_len() > 0 && list.sel < list.filtered_len() - 1 {
                        list.sel += 1;
                        clamp_list_scroll_offset(
                            list.sel,
                            list.filtered_len(),
                            MAX_VISIBLE_ROWS,
                            &mut list.scroll_offset,
                        );
                    }
                }
                KeyCode::Backspace => {
                    list.query.pop();
                    list.apply_filter(None);
                }
                KeyCode::Char(c) if !c.is_control() => {
                    list.query.push(c);
                    list.apply_filter(None);
                }
                _ => {}
            },
            Step::ConfirmDeleteProvider {
                name,
                sel,
                scroll_offset,
            } => match key_event.code {
                KeyCode::Enter | KeyCode::Char('y') | KeyCode::Char('Y') => {
                    let name = name.clone();
                    let previous_sel = *sel;
                    let previous_scroll_offset = *scroll_offset;
                    self.confirm_delete_provider(name, previous_sel, previous_scroll_offset);
                }
                KeyCode::Esc | KeyCode::Char('n') | KeyCode::Char('N') => {
                    self.go_back_to_provider_list();
                }
                _ => {}
            },
            Step::SelectWireApi { sel } => match key_event.code {
                KeyCode::Esc => {
                    self.completion = Some(ViewCompletion::Cancelled);
                }
                KeyCode::Enter => {
                    let i = *sel;
                    self.select_wire_api_row(i);
                }
                KeyCode::Up => {
                    *sel = (*sel + 2) % 3;
                }
                KeyCode::Down => {
                    *sel = (*sel + 1) % 3;
                }
                _ => {}
            },
            Step::ConfigureFields { focused_idx } => match key_event.code {
                KeyCode::Esc => {
                    self.go_back_to_provider_list();
                }
                KeyCode::Enter => {
                    self.save_and_back_to_list();
                }
                KeyCode::Char(' ') => {
                    let Some(data) = &self.config_data else {
                        return;
                    };
                    let fields = field_form_kind(data.is_custom).fields();
                    if let Some(&field) = fields.get(*focused_idx)
                        && field_is_editable(field, data.is_custom)
                    {
                        let buf = field_raw_value(field, data);
                        let cursor_pos = buf.chars().count();
                        self.step = Step::EditField {
                            field,
                            buffer: buf,
                            cursor_pos,
                        };
                        self.error = None;
                    }
                }
                KeyCode::Up => {
                    let Some(data) = &self.config_data else {
                        return;
                    };
                    let fields = field_form_kind(data.is_custom).fields();
                    if !fields.is_empty() {
                        *focused_idx = (*focused_idx + fields.len() - 1) % fields.len();
                    }
                }
                KeyCode::Down | KeyCode::Tab => {
                    let Some(data) = &self.config_data else {
                        return;
                    };
                    let fields = field_form_kind(data.is_custom).fields();
                    if !fields.is_empty() {
                        *focused_idx = (*focused_idx + 1) % fields.len();
                    }
                }
                KeyCode::BackTab => {
                    let Some(data) = &self.config_data else {
                        return;
                    };
                    let fields = field_form_kind(data.is_custom).fields();
                    if !fields.is_empty() {
                        *focused_idx = (*focused_idx + fields.len() - 1) % fields.len();
                    }
                }
                _ => {}
            },
            Step::EditField {
                field,
                buffer,
                cursor_pos,
            } => match key_event.code {
                KeyCode::Esc => {
                    self.step = Step::ConfigureFields { focused_idx: 0 };
                }
                KeyCode::Enter => {
                    let Some(data) = &mut self.config_data else {
                        return;
                    };
                    let t = buffer.trim().to_string();
                    let valid = match field {
                        ProviderFieldKind::Name => validate_name(&t, data, &self.archive),
                        ProviderFieldKind::BaseUrl => provider_ui::validate_base_url(&t),
                        ProviderFieldKind::WireApi => Ok(()),
                        ProviderFieldKind::EnvKey => Ok(()),
                        ProviderFieldKind::ApiKey => provider_ui::validate_api_key(&t),
                        ProviderFieldKind::Models => provider_ui::validate_models(&t),
                    };
                    if let Err(msg) = valid {
                        self.error = Some(msg);
                        return;
                    }
                    set_field_value(*field, data, t);
                    self.error = None;
                    // Return to ConfigureFields at the edited field, let user choose next action.
                    let fields = field_form_kind(data.is_custom).fields();
                    let pos = fields.iter().position(|f| *f == *field).unwrap_or(0);
                    self.step = Step::ConfigureFields { focused_idx: pos };
                }
                KeyCode::Backspace => {
                    if *cursor_pos > 0 {
                        // Remove character at cursor_pos - 1.
                        let char_idx = buffer
                            .char_indices()
                            .nth(*cursor_pos - 1)
                            .map(|(i, _)| i)
                            .unwrap_or(buffer.len());
                        let next_idx = buffer
                            .char_indices()
                            .nth(*cursor_pos)
                            .map(|(i, _)| i)
                            .unwrap_or(buffer.len());
                        buffer.drain(char_idx..next_idx);
                        *cursor_pos = cursor_pos.saturating_sub(1);
                    }
                }
                KeyCode::Delete => {
                    let char_count = buffer.chars().count();
                    if *cursor_pos < char_count {
                        // Remove character at cursor_pos.
                        let char_idx = buffer
                            .char_indices()
                            .nth(*cursor_pos)
                            .map(|(i, _)| i)
                            .unwrap_or(buffer.len());
                        let end_idx = buffer
                            .char_indices()
                            .nth(*cursor_pos + 1)
                            .map(|(i, _)| i)
                            .unwrap_or(buffer.len());
                        buffer.drain(char_idx..end_idx);
                    }
                }
                KeyCode::Char(c) => {
                    // Insert character at cursor_pos.
                    let chars: Vec<char> = buffer.chars().collect();
                    let mut new_chars: Vec<char> =
                        chars.iter().take(*cursor_pos).cloned().collect();
                    new_chars.push(c);
                    new_chars.extend(chars.iter().skip(*cursor_pos).cloned());
                    *buffer = new_chars.into_iter().collect();
                    *cursor_pos = cursor_pos.saturating_add(1);
                }
                KeyCode::Left => {
                    *cursor_pos = cursor_pos.saturating_sub(1);
                }
                KeyCode::Right => {
                    let len = buffer.chars().count();
                    if *cursor_pos < len {
                        *cursor_pos = cursor_pos.saturating_add(1);
                    }
                }
                KeyCode::Up => {
                    *cursor_pos = provider_ui::move_cursor_vertically_in_wrapped_text(
                        buffer,
                        *cursor_pos,
                        edit_input_width,
                        -1,
                    );
                }
                KeyCode::Down => {
                    *cursor_pos = provider_ui::move_cursor_vertically_in_wrapped_text(
                        buffer,
                        *cursor_pos,
                        edit_input_width,
                        1,
                    );
                }
                KeyCode::Home => {
                    *cursor_pos = 0;
                }
                KeyCode::End => {
                    *cursor_pos = buffer.chars().count();
                }
                _ => {}
            },
            Step::Done => {}
        }
    }

    fn handle_paste(&mut self, pasted: String) -> bool {
        if pasted.is_empty() {
            return false;
        }
        match &mut self.step {
            Step::SelectProvider(list) => {
                list.query.push_str(&pasted);
                list.apply_filter(None);
                true
            }
            Step::EditField {
                buffer, cursor_pos, ..
            } => {
                let byte_idx = buffer
                    .char_indices()
                    .nth(*cursor_pos)
                    .map(|(idx, _)| idx)
                    .unwrap_or(buffer.len());
                buffer.insert_str(byte_idx, &pasted);
                *cursor_pos += pasted.chars().count();
                true
            }
            Step::ConfigureFields { focused_idx } => {
                let Some(data) = &self.config_data else {
                    return false;
                };
                let fields = field_form_kind(data.is_custom).fields();
                let Some(&field) = fields.get(*focused_idx) else {
                    return false;
                };
                if !field_is_editable(field, data.is_custom) {
                    return false;
                }
                let cursor_pos = pasted.chars().count();
                self.step = Step::EditField {
                    field,
                    buffer: pasted,
                    cursor_pos,
                };
                self.error = None;
                true
            }
            Step::ConfirmDeleteProvider { .. } | Step::SelectWireApi { .. } | Step::Done => false,
        }
    }

    fn is_complete(&self) -> bool {
        self.completion.is_some()
    }
    fn completion(&self) -> Option<ViewCompletion> {
        self.completion
    }
    fn prefer_esc_to_handle_key_event(&self) -> bool {
        true
    }
    fn on_ctrl_c(&mut self) -> super::CancellationEvent {
        self.completion = Some(ViewCompletion::Cancelled);
        super::CancellationEvent::Handled
    }
}

// ---------------------------------------------------------------------------
// Validation helpers
// ---------------------------------------------------------------------------

fn validate_name(t: &str, data: &Data, archive: &ProviderToml) -> Result<(), String> {
    if t.is_empty() {
        return Err("name is required".into());
    }
    if archive
        .providers
        .iter()
        .any(|p| p.name == t && p.name != data.name)
    {
        return Err(format!("name '{t}' already exists"));
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Renderable
// ---------------------------------------------------------------------------

impl Renderable for ProviderWizard {
    fn render(&self, area: Rect, buf: &mut Buffer) {
        let frame_area = Rect {
            height: area.height.saturating_sub(1),
            ..area
        };
        let content = render_menu_surface(frame_area, buf);

        match &self.step {
            Step::SelectProvider(list) => self.render_provider_list(content, buf, list),
            Step::ConfirmDeleteProvider { name, .. } => {
                self.render_delete_confirmation(content, buf, name)
            }
            Step::SelectWireApi { sel } => self.render_list(
                content,
                buf,
                &wire_api_rows(),
                *sel,
                0,
                "Custom provider type",
            ),
            Step::ConfigureFields { focused_idx } => {
                self.render_configure_fields(content, buf, *focused_idx)
            }
            Step::EditField {
                field,
                buffer,
                cursor_pos,
            } => self.render_edit_field(content, buf, *field, buffer, *cursor_pos),
            Step::Done => {}
        }

        if area.height > 0 {
            self.footer_hint_line().dim().render(
                Rect {
                    y: area.y.saturating_add(area.height.saturating_sub(1)),
                    height: 1,
                    ..area
                },
                buf,
            );
        }
    }

    fn desired_height(&self, width: u16) -> u16 {
        let content_width = width.saturating_sub(4).max(1);
        let inner = match &self.step {
            Step::SelectProvider(list) => list.filtered_len().min(MAX_VISIBLE_ROWS) + 5,
            Step::ConfirmDeleteProvider { .. } => 5,
            Step::SelectWireApi { .. } => 6,
            Step::ConfigureFields { .. } => self.config_data.as_ref().map_or(0, |d| {
                let title = configure_title(d);
                let title_height = provider_ui::wrapped_line_count(&title, content_width);
                let value_width = content_width.saturating_sub(14).max(1);
                let fields_height = field_form_kind(d.is_custom)
                    .fields()
                    .iter()
                    .map(|field| {
                        let value = field_display_value(*field, d);
                        let display = if value.is_empty() {
                            "—".to_string()
                        } else {
                            value
                        };
                        provider_ui::wrapped_line_count(&display, value_width)
                    })
                    .sum::<usize>();
                title_height + 1 + fields_height + 2
            }),
            Step::EditField { buffer, .. } => {
                4 + provider_ui::input_frame_height(buffer, content_width) as usize
            }
            Step::Done => 1,
        };
        (inner + 2) as u16 + 1
    }

    fn cursor_pos(&self, area: Rect) -> Option<(u16, u16)> {
        let Step::EditField {
            buffer, cursor_pos, ..
        } = &self.step
        else {
            return None;
        };
        let frame_area = Rect {
            height: area.height.saturating_sub(1),
            ..area
        };
        let content = menu_surface_inset(frame_area);
        if content.height < 4 {
            return None;
        }
        let input_area = Rect {
            x: content.x,
            y: content.y.saturating_add(2),
            width: content.width.max(1),
            height: content.height.saturating_sub(2),
        };
        let text_area = provider_ui::input_frame_text_area(input_area)?;
        self.edit_input_width.set(text_area.width.max(1));
        provider_ui::cursor_pos_for_wrapped_text(buffer, *cursor_pos, text_area)
    }
}

// ── List rendering ──────────────────────────────────────────────────

impl ProviderWizard {
    fn footer_hint_line(&self) -> Line<'static> {
        match &self.step {
            Step::SelectProvider(_) => Line::from(vec![
                "type".cyan(),
                " search · ".dim(),
                "up/down".cyan(),
                " navigate · ".dim(),
                "enter".cyan(),
                " switch · ".dim(),
                "space".cyan(),
                " edit · ".dim(),
                "ctrl+d/del".cyan(),
                " delete · ".dim(),
                "esc".cyan(),
                " clear/close".dim(),
            ]),
            Step::ConfirmDeleteProvider { .. } => Line::from(vec![
                "enter/y".cyan(),
                " delete · ".dim(),
                "esc/n".cyan(),
                " keep".dim(),
            ]),
            Step::SelectWireApi { .. } => Line::from(vec![
                "up/down".cyan(),
                " navigate · ".dim(),
                "enter".cyan(),
                " choose · ".dim(),
                "esc".cyan(),
                " back".dim(),
            ]),
            Step::ConfigureFields { .. } => Line::from(vec![
                "up/down".cyan(),
                " navigate · ".dim(),
                "enter".cyan(),
                " save · ".dim(),
                "space".cyan(),
                " edit · ".dim(),
                "esc".cyan(),
                " back".dim(),
            ]),
            Step::EditField { .. } => Line::from(vec![
                "enter".cyan(),
                " save · ".dim(),
                "esc".cyan(),
                " cancel".dim(),
            ]),
            Step::Done => Line::from(""),
        }
    }

    fn render_list(
        &self,
        area: Rect,
        buf: &mut Buffer,
        rows: &[PRow],
        sel: usize,
        scroll_offset: usize,
        title: &str,
    ) {
        if area.height < 1 {
            return;
        }
        let lr = |y| Rect {
            x: area.x,
            y,
            width: area.width.max(1),
            height: 1,
        };
        let mut y = area.y;
        let bot = area.y.saturating_add(area.height);

        let max_visible = MAX_VISIBLE_ROWS;
        if y < bot {
            Paragraph::new(Line::from(Span::from(title).bold())).render(lr(y), buf);
            y = y.saturating_add(1);
        }
        if y < bot {
            y = y.saturating_add(1);
        }

        // Rows with scroll_offset. Widths are cached in PRow so repeated
        // up/down key renders only touch the visible rows.
        let max_num_width = rows.len().max(1).to_string().width();
        let max_name_width = rows
            .iter()
            .skip(scroll_offset)
            .take(max_visible)
            .map(|r| r.name_width)
            .max()
            .unwrap_or(0)
            .max(16);
        let visible_items = rows
            .iter()
            .enumerate()
            .skip(scroll_offset)
            .take(max_visible);
        for (i, r) in visible_items {
            if y >= bot {
                break;
            }
            let is_sel = i == sel;
            let prefix = if is_sel { "› " } else { "  " };
            let num = format!("{:>max_num_width$}. ", i + 1);
            let name_pad = max_name_width.saturating_sub(r.name_width);
            let padded = format!("{}{}", r.name, " ".repeat(name_pad));
            let name: Span = if is_sel {
                Span::from(padded).cyan().bold()
            } else {
                Span::from(padded)
            };
            let desc: Span = if r.desc.is_empty() {
                Span::from("")
            } else if is_sel {
                Span::from(format!("  {}", r.desc)).cyan().bold()
            } else {
                Span::from(format!("  {}", r.desc)).dim()
            };
            Paragraph::new(Line::from(vec![
                if is_sel {
                    Span::from(prefix).cyan().bold()
                } else {
                    Span::from(prefix)
                },
                if is_sel {
                    Span::from(num).cyan().bold()
                } else {
                    Span::from(num)
                },
                name,
                desc,
            ]))
            .render(lr(y), buf);
            y = y.saturating_add(1);
        }

        if y < bot {
            y = y.saturating_add(1);
        }
        if let Some(err) = &self.error
            && y < bot
        {
            Paragraph::new(Line::from(vec![
                "✖".red().bold(),
                "  ".into(),
                Span::from(err.clone()).red(),
            ]))
            .render(lr(y), buf);
        }
    }

    fn render_provider_list(&self, area: Rect, buf: &mut Buffer, list: &ProviderListState) {
        if area.height < 1 {
            return;
        }
        let lr = |y| Rect {
            x: area.x,
            y,
            width: area.width.max(1),
            height: 1,
        };
        let mut y = area.y;
        let bot = area.y.saturating_add(area.height);

        if y < bot {
            Paragraph::new(Line::from(Span::from("Choose a provider").bold())).render(lr(y), buf);
            y = y.saturating_add(1);
        }
        if y < bot
            && let Some(active) = &self.active_provider_name
        {
            Paragraph::new(Line::from(Span::from(format!("active: {active}")).dim()))
                .render(lr(y), buf);
            y = y.saturating_add(1);
        }
        if y < bot {
            let query = if list.query.is_empty() {
                "type to search providers".to_string()
            } else {
                list.query.clone()
            };
            let query_span = if list.query.is_empty() {
                Span::from(query).dim()
            } else {
                Span::from(query).cyan()
            };
            Paragraph::new(Line::from(vec!["search: ".dim(), query_span])).render(lr(y), buf);
            y = y.saturating_add(1);
        }
        if y < bot {
            y = y.saturating_add(1);
        }

        let visible_indices = list
            .filtered_indices
            .iter()
            .enumerate()
            .skip(list.scroll_offset)
            .take(MAX_VISIBLE_ROWS);
        let max_num_width = list.filtered_len().max(1).to_string().width();
        let max_name_width = list
            .filtered_indices
            .iter()
            .skip(list.scroll_offset)
            .take(MAX_VISIBLE_ROWS)
            .filter_map(|&row_idx| list.rows.get(row_idx))
            .map(|row| row.name_width)
            .max()
            .unwrap_or(0)
            .max(16);

        for (filtered_idx, row_idx) in visible_indices {
            if y >= bot {
                break;
            }
            let Some(row) = list.rows.get(*row_idx) else {
                continue;
            };
            let is_sel = filtered_idx == list.sel;
            let prefix = if is_sel { "› " } else { "  " };
            let num = format!("{:>max_num_width$}. ", filtered_idx + 1);
            let name_pad = max_name_width.saturating_sub(row.name_width);
            let padded = format!("{}{}", row.name, " ".repeat(name_pad));
            let name: Span = if is_sel {
                Span::from(padded).cyan().bold()
            } else {
                Span::from(padded)
            };
            let desc: Span = if row.desc.is_empty() {
                Span::from("")
            } else if is_sel {
                Span::from(format!("  {}", row.desc)).cyan().bold()
            } else {
                Span::from(format!("  {}", row.desc)).dim()
            };
            Paragraph::new(Line::from(vec![
                if is_sel {
                    Span::from(prefix).cyan().bold()
                } else {
                    Span::from(prefix)
                },
                if is_sel {
                    Span::from(num).cyan().bold()
                } else {
                    Span::from(num)
                },
                name,
                desc,
            ]))
            .render(lr(y), buf);
            y = y.saturating_add(1);
        }

        if list.filtered_indices.is_empty() && y < bot {
            Paragraph::new(Line::from("No providers match your search".dim())).render(lr(y), buf);
            y = y.saturating_add(1);
        }

        if y < bot {
            y = y.saturating_add(1);
        }
        if let Some(err) = &self.error
            && y < bot
        {
            Paragraph::new(Line::from(vec![
                "✖".red().bold(),
                "  ".into(),
                Span::from(err.clone()).red(),
            ]))
            .render(lr(y), buf);
        }
    }

    fn render_delete_confirmation(&self, area: Rect, buf: &mut Buffer, name: &str) {
        if area.height < 1 {
            return;
        }
        let lr = |y| Rect {
            x: area.x,
            y,
            width: area.width.max(1),
            height: 1,
        };
        let mut y = area.y;
        let bot = area.y.saturating_add(area.height);

        if y < bot {
            Paragraph::new(Line::from("Delete provider?".bold())).render(lr(y), buf);
            y = y.saturating_add(2);
        }
        if y < bot {
            Paragraph::new(Line::from(vec!["Provider ".into(), name.red().bold()]))
                .render(lr(y), buf);
            y = y.saturating_add(1);
        }
        if y < bot {
            Paragraph::new(Line::from("This removes it from provider.toml only.".dim()))
                .render(lr(y), buf);
        }
    }

    // ── Configure fields ──────────────────────────────────────────────

    fn render_configure_fields(&self, area: Rect, buf: &mut Buffer, focused_idx: usize) {
        if area.height < 1 {
            return;
        }
        let data = match &self.config_data {
            Some(d) => d,
            None => return,
        };
        let lr = |y| Rect {
            x: area.x,
            y,
            width: area.width.max(1),
            height: 1,
        };
        let mut y = area.y;
        let bot = area.y.saturating_add(area.height);

        let badge = if data.is_custom { "CUSTOM" } else { "REMOTE" };
        if y < bot {
            let title_height =
                provider_ui::wrapped_line_count(&configure_title(data), area.width) as u16;
            Paragraph::new(Line::from(vec![
                "◆ ".green().bold(),
                Span::from(format!(
                    "Configure {}",
                    if data.name.is_empty() {
                        "New provider"
                    } else {
                        &data.name
                    }
                ))
                .bold(),
                "   ".into(),
                Span::from("[").dim(),
                if data.is_custom {
                    Span::from(badge).cyan().bold()
                } else {
                    Span::from(badge).green().bold()
                },
                Span::from("]").dim(),
            ]))
            .wrap(Wrap { trim: false })
            .render(
                Rect {
                    height: title_height,
                    ..lr(y)
                },
                buf,
            );
            y = y.saturating_add(title_height);
        }
        if y < bot {
            y = y.saturating_add(1);
        }

        for (fi, &field) in field_form_kind(data.is_custom).fields().iter().enumerate() {
            if y >= bot {
                break;
            }
            let consumed = self.render_field_row(lr(y), buf, field, data, fi == focused_idx, bot);
            y = y.saturating_add(consumed);
        }

        if let Some(err) = &self.error {
            if y + 1 < bot {
                y = y.saturating_add(1);
            }
            if y < bot {
                Paragraph::new(Line::from(vec![
                    "✖".red().bold(),
                    "  ".into(),
                    Span::from(err.clone()).red(),
                ]))
                .render(lr(y), buf);
            }
        }
    }

    fn render_field_row(
        &self,
        rect: Rect,
        buf: &mut Buffer,
        field: ProviderFieldKind,
        data: &Data,
        focused: bool,
        bot: u16,
    ) -> u16 {
        let lw = provider_ui::PROVIDER_LABEL_WIDTH;
        let val = field_display_value(field, data);
        let vd = if val.is_empty() {
            "—".to_string()
        } else {
            val
        };
        let editable = field_is_editable(field, data.is_custom);
        let value_width = rect.width.saturating_sub((lw + 2) as u16).max(1);
        let ranges = provider_ui::wrap_ranges(&vd, value_width);

        for (line_idx, range) in ranges.iter().enumerate() {
            let y = rect.y.saturating_add(line_idx as u16);
            if y >= bot {
                break;
            }
            let cur = if focused && line_idx == 0 {
                "›".cyan().bold()
            } else {
                " ".into()
            };
            let prefix = if line_idx == 0 {
                Span::from(format!("{:<lw$}", field.label())).dim()
            } else {
                Span::from(" ".repeat(lw)).dim()
            };
            let value = &vd[range.clone()];
            let vs = if focused {
                Span::from(value.to_string()).cyan().bold()
            } else if editable {
                Span::from(value.to_string())
            } else {
                Span::from(value.to_string()).dim()
            };
            let mut spans = vec![cur, " ".into(), prefix, vs];
            if line_idx == 0 && focused && editable {
                spans.push(Span::from("  space to edit").cyan().dim());
            } else if line_idx == 0 && !editable {
                spans.push(Span::from("  · readonly").dim().italic());
            }
            Paragraph::new(Line::from(spans)).render(Rect { y, ..rect }, buf);
        }
        ranges.len() as u16
    }

    // ── Edit field ─────────────────────────────────────────────────────

    fn render_edit_field(
        &self,
        area: Rect,
        buf: &mut Buffer,
        field: ProviderFieldKind,
        buffer: &str,
        _cursor_pos: usize,
    ) {
        if area.height < 1 {
            return;
        }
        let lr = |y| Rect {
            x: area.x,
            y,
            width: area.width.max(1),
            height: 1,
        };
        let mut y = area.y;
        let bot = area.y.saturating_add(area.height);
        let label = field.label();

        if y < bot {
            Paragraph::new(Line::from(vec![
                "◆ ".green().bold(),
                Span::from(format!("Edit {label}")).bold(),
            ]))
            .render(lr(y), buf);
            y = y.saturating_add(1);
        }
        if y < bot {
            y = y.saturating_add(1);
        }

        // Input text; the terminal cursor is positioned via `cursor_pos`.
        if y < bot {
            let input_height = provider_ui::input_frame_height(buffer, area.width)
                .min(bot.saturating_sub(y))
                .max(1);
            let input_area = Rect {
                x: area.x,
                y,
                width: area.width.max(1),
                height: input_height,
            };
            if let Some(text_area) = provider_ui::input_frame_text_area(input_area) {
                self.edit_input_width.set(text_area.width.max(1));
            }
            provider_ui::render_input_frame(input_area, buf, buffer, true);
            y = y.saturating_add(input_height);
        }
        // Gap between input and hint.
        if y < bot {
            y = y.saturating_add(1);
        }
        let hint = match field {
            ProviderFieldKind::Name
            | ProviderFieldKind::BaseUrl
            | ProviderFieldKind::WireApi
            | ProviderFieldKind::EnvKey
            | ProviderFieldKind::ApiKey
            | ProviderFieldKind::Models => field.help(),
        };
        if y < bot {
            Paragraph::new(Line::from(vec![Span::from(hint).dim().italic()])).render(lr(y), buf);
            y = y.saturating_add(1);
        }
        if let Some(err) = &self.error
            && y < bot
        {
            Paragraph::new(Line::from(vec![
                "✖".red().bold(),
                "  ".into(),
                Span::from(err.clone()).red(),
            ]))
            .render(lr(y), buf);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use insta::assert_snapshot;
    use ratatui::buffer::Buffer;
    use tokio::sync::mpsc::unbounded_channel;

    fn test_wizard(step: Step, config_data: Option<Data>) -> ProviderWizard {
        let (tx, _rx) = unbounded_channel();
        let agere_home =
            agere_utils_fs::AbsolutePathBuf::from_absolute_path(std::env::current_dir().unwrap())
                .expect("current dir should be absolute");
        ProviderWizard {
            archive: ProviderToml::default(),
            builtin_templates: Vec::new(),
            remote_templates: None,
            active_provider_name: None,
            step,
            config_data,
            error: None,
            completion: None,
            app_event_tx: AppEventSender::new(tx),
            agere_home,
            edit_input_width: Cell::new(1),
        }
    }

    fn render_snapshot(view: &ProviderWizard, width: u16) -> String {
        let area = Rect::new(0, 0, width, view.desired_height(width));
        let mut buf = Buffer::empty(area);
        view.render(area, &mut buf);
        format!("{buf:?}")
    }

    #[test]
    fn remote_configure_fields_wrap_to_available_width() {
        let data = Data {
            name: "SiliconFlow (China)".to_string(),
            base_url: "https://api.siliconflow.cn/v1".to_string(),
            wire_api: WireApi::Chat,
            env_key: "SILICONFLOW_API_KEY".to_string(),
            api_key: "sk-siliconflow-example-key".to_string(),
            models_str: "deepseek-ai/DeepSeek-V3,deepseek-ai/DeepSeek-R1,Qwen/Qwen3-Coder-480B-A35B-Instruct".to_string(),
            is_custom: false,
        };
        let view = test_wizard(Step::ConfigureFields { focused_idx: 1 }, Some(data));

        assert_snapshot!(
            "provider_wizard_remote_configure_fields_wrap_to_available_width",
            render_snapshot(&view, 34)
        );
    }

    #[test]
    fn custom_configure_fields_wrap_to_available_width() {
        let data = Data {
            name: "Very long custom provider for local testing".to_string(),
            base_url: "https://gateway.example.internal/openai-compatible/v1".to_string(),
            wire_api: WireApi::Chat,
            env_key: "CUSTOM_PROVIDER_API_KEY".to_string(),
            api_key: "sk-custom-provider-example-key".to_string(),
            models_str: "custom-chat-model-with-a-long-name[256k],custom-reasoning-model[1m]"
                .to_string(),
            is_custom: true,
        };
        let view = test_wizard(Step::ConfigureFields { focused_idx: 1 }, Some(data));

        assert_snapshot!(
            "provider_wizard_custom_configure_fields_wrap_to_available_width",
            render_snapshot(&view, 38)
        );
    }

    #[test]
    fn edit_field_wraps_buffer_to_available_width() {
        let view = test_wizard(
            Step::EditField {
                field: ProviderFieldKind::Models,
                buffer: "deepseek-ai/DeepSeek-V3,deepseek-ai/DeepSeek-R1,Qwen/Qwen3-Coder-480B-A35B-Instruct".to_string(),
                cursor_pos: 50,
            },
            None,
        );

        assert_snapshot!(
            "provider_wizard_edit_field_wraps_buffer_to_available_width",
            render_snapshot(&view, 34)
        );
    }

    #[test]
    fn edit_field_cursor_tracks_rendered_input_row() {
        let view = test_wizard(
            Step::EditField {
                field: ProviderFieldKind::Name,
                buffer: "kk".to_string(),
                cursor_pos: 2,
            },
            None,
        );
        let area = Rect::new(0, 0, 105, view.desired_height(105));

        assert_eq!(view.cursor_pos(area), Some((6, 3)));
    }

    #[test]
    fn edit_field_up_down_moves_across_wrapped_lines() {
        let mut view = test_wizard(
            Step::EditField {
                field: ProviderFieldKind::Models,
                buffer: "abcdefghi".to_string(),
                cursor_pos: 7,
            },
            None,
        );
        view.edit_input_width.set(3);

        view.handle_key_event(KeyEvent::from(KeyCode::Up));
        let Step::EditField { cursor_pos, .. } = view.step else {
            panic!("expected edit field");
        };
        assert_eq!(cursor_pos, 4);

        view.handle_key_event(KeyEvent::from(KeyCode::Down));
        let Step::EditField { cursor_pos, .. } = view.step else {
            panic!("expected edit field");
        };
        assert_eq!(cursor_pos, 7);
    }

    #[test]
    fn edit_api_key_field_accepts_paste_at_cursor() {
        let mut view = test_wizard(
            Step::EditField {
                field: ProviderFieldKind::ApiKey,
                buffer: "sk--suffix".to_string(),
                cursor_pos: 3,
            },
            None,
        );

        assert!(view.handle_paste("secret".to_string()));

        let Step::EditField {
            buffer, cursor_pos, ..
        } = view.step
        else {
            panic!("expected edit field");
        };
        assert_eq!(buffer, "sk-secret-suffix");
        assert_eq!(cursor_pos, 9);
    }

    #[test]
    fn configure_fields_pastes_into_focused_editable_field() {
        let data = Data::new_custom(WireApi::Responses, "https://old.example/v1");
        let mut view = test_wizard(Step::ConfigureFields { focused_idx: 1 }, Some(data));

        assert!(view.handle_paste("https://new.example/v1".to_string()));

        let Step::EditField {
            field,
            buffer,
            cursor_pos,
        } = view.step
        else {
            panic!("expected edit field");
        };
        assert_eq!(field, ProviderFieldKind::BaseUrl);
        assert_eq!(buffer, "https://new.example/v1");
        assert_eq!(cursor_pos, 22);
    }

    #[test]
    fn provider_list_first_screen_uses_compact_popup_layout() {
        let rows = vec![
            PRow {
                name: "Add custom provider".to_string(),
                desc: "create a provider from scratch".to_string(),
                name_width: "Add custom provider".width(),
                kind: PRowKind::AddCustom,
            },
            PRow {
                name: "kk-openai".to_string(),
                desc: "current session default".to_string(),
                name_width: "kk-openai".width(),
                kind: PRowKind::SavedComplete("kk-openai".to_string()),
            },
            PRow {
                name: "deepseek".to_string(),
                desc: "template — add API key to enable".to_string(),
                name_width: "deepseek".width(),
                kind: PRowKind::NeedConfig(Data::default()),
            },
        ];
        let mut view = test_wizard(
            Step::SelectProvider(ProviderListState::new(rows, Some("kk-openai"))),
            None,
        );
        view.active_provider_name = Some("kk-openai".to_string());

        assert_snapshot!(
            "provider_wizard_list_first_screen_summary",
            render_snapshot(&view, 78)
        );
    }

    #[test]
    fn remote_template_refresh_keeps_selected_row_visible() {
        let rows = (0..12)
            .map(|i| PRow {
                name: format!("provider-{i}"),
                desc: String::new(),
                name_width: format!("provider-{i}").width(),
                kind: PRowKind::NeedConfig(Data::default()),
            })
            .collect();
        let mut list = ProviderListState::new(rows, None);
        list.sel = 11;
        list.scroll_offset = 3;
        let mut view = test_wizard(Step::SelectProvider(list), None);

        view.update_remote_templates(vec![ProviderTemplate {
            name: "custom-template".to_string(),
            base_url: "https://api.example.test/v1".to_string(),
            wire_api: WireApi::Chat,
            env_key: None,
            models: Vec::new(),
        }]);

        let Step::SelectProvider(list) = view.step else {
            panic!("expected provider list");
        };
        assert!(!list.rows.is_empty());
        assert!(list.sel < list.filtered_len());
        assert!(list.sel >= list.scroll_offset);
        assert!(list.sel < list.scroll_offset + MAX_VISIBLE_ROWS);
    }

    #[test]
    fn typing_filters_provider_list_by_name_and_esc_clears_search() {
        let rows = vec![
            PRow {
                name: "Add custom provider".to_string(),
                desc: "create a provider from scratch".to_string(),
                name_width: "Add custom provider".width(),
                kind: PRowKind::AddCustom,
            },
            PRow {
                name: "deepseek".to_string(),
                desc: "template — add API key to enable".to_string(),
                name_width: "deepseek".width(),
                kind: PRowKind::NeedConfig(Data {
                    base_url: "https://api.deepseek.com/v1".to_string(),
                    models_str: "deepseek-chat".to_string(),
                    ..Data::default()
                }),
            },
            PRow {
                name: "anthropic".to_string(),
                desc: "template — add API key to enable".to_string(),
                name_width: "anthropic".width(),
                kind: PRowKind::NeedConfig(Data {
                    base_url: "https://api.anthropic.com".to_string(),
                    models_str: "claude-sonnet".to_string(),
                    ..Data::default()
                }),
            },
        ];
        let mut view = test_wizard(
            Step::SelectProvider(ProviderListState::new(rows, None)),
            None,
        );

        for c in "ANTH".chars() {
            view.handle_key_event(KeyEvent::from(KeyCode::Char(c)));
        }

        let Step::SelectProvider(list) = &view.step else {
            panic!("expected provider list");
        };
        assert_eq!(list.query, "ANTH");
        assert_eq!(list.filtered_len(), 1);
        assert_eq!(
            list.selected_row().map(|row| row.name.as_str()),
            Some("anthropic")
        );

        view.handle_key_event(KeyEvent::from(KeyCode::Esc));

        let Step::SelectProvider(list) = &view.step else {
            panic!("expected provider list");
        };
        assert!(list.query.is_empty());
        assert_eq!(list.filtered_len(), 3);
        assert_eq!(view.completion(), None);
    }

    #[test]
    fn provider_search_ignores_non_name_fields() {
        let rows = vec![
            PRow {
                name: "xAI".to_string(),
                desc: "template — add API key to enable".to_string(),
                name_width: "xAI".width(),
                kind: PRowKind::NeedConfig(Data {
                    base_url: "https://api.x.ai/v1".to_string(),
                    env_key: "XAI_API_KEY".to_string(),
                    models_str: "grok-imagine-image-quality".to_string(),
                    ..Data::default()
                }),
            },
            PRow {
                name: "Alibaba Cloud".to_string(),
                desc: "template — add API key to enable".to_string(),
                name_width: "Alibaba Cloud".width(),
                kind: PRowKind::NeedConfig(Data {
                    base_url: "https://dashscope.aliyuncs.com/compatible-mode/v1".to_string(),
                    env_key: "DASHSCOPE_API_KEY".to_string(),
                    models_str: "qwen-plus".to_string(),
                    ..Data::default()
                }),
            },
        ];
        let mut list = ProviderListState::new(rows, None);
        list.query = "ali".to_string();
        list.apply_filter(None);

        assert_eq!(list.filtered_len(), 1);
        assert_eq!(
            list.selected_row().map(|row| row.name.as_str()),
            Some("Alibaba Cloud")
        );
    }

    #[test]
    fn save_trims_provider_fields_before_writing() {
        let tmp = tempfile::tempdir().expect("tmpdir");
        let agere_home = agere_utils_fs::AbsolutePathBuf::from_absolute_path(tmp.path())
            .expect("tmpdir should be absolute");
        let (tx, _rx) = unbounded_channel();
        let mut view = ProviderWizard {
            archive: ProviderToml::default(),
            builtin_templates: Vec::new(),
            remote_templates: None,
            active_provider_name: None,
            step: Step::ConfigureFields { focused_idx: 0 },
            config_data: Some(Data {
                name: " custom-provider ".to_string(),
                base_url: " https://api.example.test/v1 ".to_string(),
                wire_api: WireApi::Chat,
                env_key: " CUSTOM_PROVIDER_API_KEY ".to_string(),
                api_key: " sk-test ".to_string(),
                models_str: " model-a[128k], model-b ".to_string(),
                is_custom: true,
            }),
            error: None,
            completion: None,
            app_event_tx: AppEventSender::new(tx),
            agere_home: agere_home.clone(),
            edit_input_width: Cell::new(1),
        };

        view.save_and_back_to_list();

        assert_eq!(view.error, None);
        let loaded = crate::onboarding::provider_toml::load(&agere_home);
        assert_eq!(loaded.providers.len(), 1);
        assert_eq!(loaded.providers[0].name, "custom-provider");
        assert_eq!(loaded.providers[0].base_url, "https://api.example.test/v1");
        assert_eq!(
            loaded.providers[0].env_key,
            Some("CUSTOM_PROVIDER_API_KEY".to_string())
        );
        assert_eq!(loaded.providers[0].get_api_key(&agere_home), "sk-test");
        assert_eq!(
            loaded.providers[0].models,
            vec![
                agere_config::config_toml::ModelConfig {
                    name: "model-a".to_string(),
                    context_window: Some(128_000),
                    input_modalities: None,
                },
                agere_config::config_toml::ModelConfig {
                    name: "model-b".to_string(),
                    context_window: None,
                    input_modalities: None,
                },
            ]
        );
    }

    #[test]
    fn save_reports_invalid_model_context() {
        let mut view = test_wizard(
            Step::ConfigureFields { focused_idx: 0 },
            Some(Data {
                name: "custom-provider".to_string(),
                base_url: "https://api.example.test/v1".to_string(),
                wire_api: WireApi::Chat,
                api_key: "sk-test".to_string(),
                models_str: "model-a[bad]".to_string(),
                is_custom: true,
                ..Data::default()
            }),
        );

        view.save_and_back_to_list();

        assert_eq!(
            view.error,
            Some(
                "invalid context window for model 'model-a': use digits with optional k, m, or g"
                    .to_string()
            )
        );
    }

    #[test]
    fn esc_from_custom_provider_choice_closes_wizard() {
        let rows = vec![PRow {
            name: "Add custom provider".to_string(),
            desc: "create a provider from scratch".to_string(),
            name_width: "Add custom provider".width(),
            kind: PRowKind::AddCustom,
        }];
        let mut view = test_wizard(
            Step::SelectProvider(ProviderListState::new(rows, None)),
            None,
        );

        view.handle_key_event(KeyEvent::from(KeyCode::Enter));
        view.handle_key_event(KeyEvent::from(KeyCode::Esc));

        assert_eq!(view.completion(), Some(ViewCompletion::Cancelled));
    }
}
