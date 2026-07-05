//! Single-page provider picker shown by the onboarding welcome screen.
//!
//! The widget is owned by [`OnboardingScreen`]. It owns the merged list of
//! `provider.toml` archive entries + remote / built-in templates, and surfaces a
//! [`SelectResult`] when the user makes a choice. The chat `/provider` command
//! uses the bottom-pane `ProviderWizard` instead.

use std::collections::HashSet;
use std::path::PathBuf;

use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyEventKind;
use crossterm::event::KeyModifiers;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::prelude::Widget;
use ratatui::style::Stylize;
use ratatui::text::Line;
use ratatui::text::Span;
use ratatui::widgets::Clear;
use ratatui::widgets::WidgetRef;
use unicode_width::UnicodeWidthStr;

use crate::live_wrap::take_prefix_by_width;
use crate::onboarding::onboarding_screen::KeyboardHandler;
use crate::onboarding::onboarding_screen::StepState;
use crate::onboarding::onboarding_screen::StepStateProvider;
use crate::onboarding::provider_templates::ProviderTemplate;
use crate::onboarding::provider_templates::TemplateLoadState;
use crate::onboarding::provider_toml::ProviderEntry;
use crate::onboarding::provider_toml::ProviderToml;
use crate::onboarding::welcome_frame;
use crate::provider_ui;
use agere_model_provider_info::BUILT_IN_PROVIDERS;

/// Built-in providers that are intentionally hidden from the picker because
/// they require non-bearer auth.
const HIDDEN_BUILTINS: &[&str] = BUILT_IN_PROVIDERS;

const PROVIDER_ROW_HEIGHT: u16 = 2;
const KEY_EDITOR_HEIGHT: u16 = 3;

/// Mask: first 4 chars + bullets + last 4 chars. No ellipsis.
/// For short keys (≤ 8): all bullets.
fn mask_key_with_ends(key: &str) -> String {
    let len = key.chars().count();
    if len <= 8 {
        return "•".repeat(len);
    }
    let head: String = key.chars().take(4).collect();
    let tail: String = key.chars().skip(len - 4).collect();
    let middle = len - 8;
    format!("{head}{}{tail}", "•".repeat(middle))
}

fn inline_key_display_value(buffer: &str) -> String {
    if buffer.is_empty() {
        "Paste or type API key…".to_string()
    } else {
        mask_key_with_ends(buffer)
    }
}

#[derive(Clone, Debug)]
pub(crate) enum SelectResult {
    Cancel,
    /// Proceed to next step (TrustDirectory) and switch to this provider.
    ProceedToNext {
        name: String,
    },
    /// Save API key to provider.toml but stay on picker (no switch to config.toml yet).
    SaveKey {
        name: String,
        api_key: String,
    },
    DeleteRequested(String),
    /// Open the custom provider configuration page (triggered by ESC on the welcome screen).
    OpenCustomProvider,
}

#[derive(Clone, Debug)]
enum ItemKind {
    AddCustom,
    /// Already saved provider with valid api_key (green).
    CompleteSaved(ProviderEntry),
    /// Saved provider but api_key missing (red).
    IncompleteSaved(ProviderEntry),
    /// Remote/built-in template with no local record (default color).
    Template(ProviderTemplate),
}

impl ItemKind {
    fn display_name(&self) -> &str {
        match self {
            ItemKind::AddCustom => "Add custom provider",
            ItemKind::CompleteSaved(e) | ItemKind::IncompleteSaved(e) => &e.name,
            ItemKind::Template(t) => &t.name,
        }
    }

    fn base_url(&self) -> &str {
        match self {
            ItemKind::AddCustom => "create a provider from scratch",
            ItemKind::CompleteSaved(e) | ItemKind::IncompleteSaved(e) => &e.base_url,
            ItemKind::Template(t) => &t.base_url,
        }
    }

    fn is_custom(&self) -> bool {
        match self {
            ItemKind::AddCustom => false,
            ItemKind::CompleteSaved(e) | ItemKind::IncompleteSaved(e) => e.is_custom,
            ItemKind::Template(_) => false,
        }
    }
}

/// Inline input mode state for entering API key without leaving the picker.
#[derive(Clone, Debug, Default)]
enum InlineInputState {
    /// Normal list navigation mode.
    #[default]
    ListNavigation,
    /// Inline input mode: a provider is selected and the user is entering its API key.
    InlineInput {
        provider_name: String,
        buffer: String,
        /// Original decrypted key when editing a saved provider. The edit
        /// buffer is intentionally prefilled with the same value so saving
        /// without changes preserves the provider's key.
        existing_key: String,
    },
}

pub(crate) struct ProviderSelect {
    archive: ProviderToml,
    agere_home: PathBuf,
    builtin_templates: Vec<ProviderTemplate>,
    remote_state: TemplateLoadState,
    active_provider_name: Option<String>,
    filter: String,
    selected_idx: usize,
    /// Index of the first visible item in the list (for pagination/scrolling).
    scroll_offset: usize,
    delete_confirm: Option<String>,
    result: Option<SelectResult>,
    is_done: bool,
    /// When true, render the welcome brand card above the list.
    show_welcome: bool,
    /// Inline input state: either navigating the list or entering an API key.
    inline_input_state: InlineInputState,
}

impl ProviderSelect {
    pub(crate) fn new(
        archive: ProviderToml,
        agere_home: PathBuf,
        builtin_templates: Vec<ProviderTemplate>,
        active_provider_name: Option<String>,
    ) -> Self {
        let templates = builtin_templates.clone();
        let mut select = Self {
            archive,
            agere_home,
            builtin_templates,
            // Default to "Loaded(builtins)" so we don't render a permanent
            // "Loading templates..." line when no real fetch is in flight.
            remote_state: TemplateLoadState::Loaded(templates),
            active_provider_name,
            filter: String::new(),
            selected_idx: 0,
            scroll_offset: 0,
            delete_confirm: None,
            result: None,
            is_done: false,
            show_welcome: false,
            inline_input_state: InlineInputState::default(),
        };
        // Snap the selection to the active provider if it's present.
        select.snap_focus_to_active();
        select
    }

    pub(crate) fn enable_welcome_banner(&mut self) {
        self.show_welcome = true;
    }

    pub(crate) fn mark_done(&mut self) {
        self.is_done = true;
    }

    pub(crate) fn set_remote_state(&mut self, state: TemplateLoadState) {
        self.remote_state = state;
    }

    pub(crate) fn take_remote_state(&mut self) -> TemplateLoadState {
        std::mem::replace(&mut self.remote_state, TemplateLoadState::Loading)
    }

    /// Search for a provider template by name across loaded remote templates
    /// and built-in templates.  Used by `OnboardingScreen::persist_inline_key`
    /// so that remote-only providers (not in builtins) can also be saved.
    pub(crate) fn find_template(&self, name: &str) -> Option<&ProviderTemplate> {
        if let TemplateLoadState::Loaded(remote) = &self.remote_state
            && let Some(t) = remote.iter().find(|t| t.name == name)
        {
            return Some(t);
        }
        self.builtin_templates.iter().find(|t| t.name == name)
    }

    pub(crate) fn take_result(&mut self) -> Option<SelectResult> {
        self.result.take()
    }

    pub(crate) fn known_names(&self) -> HashSet<String> {
        let mut out: HashSet<String> = self
            .archive
            .providers
            .iter()
            .map(|p| p.name.clone())
            .collect();
        for t in self.builtin_templates.iter() {
            out.insert(t.name.clone());
        }
        if let TemplateLoadState::Loaded(remote) = &self.remote_state {
            for t in remote {
                out.insert(t.name.clone());
            }
        }
        out
    }

    fn templates(&self) -> &[ProviderTemplate] {
        match &self.remote_state {
            TemplateLoadState::Loaded(remote) => remote.as_slice(),
            _ => self.builtin_templates.as_slice(),
        }
    }

    fn build_items(&self) -> Vec<ItemKind> {
        let mut items: Vec<ItemKind> = Vec::new();
        items.push(ItemKind::AddCustom);
        let archive_names: HashSet<String> = self
            .archive
            .providers
            .iter()
            .map(|p| p.name.clone())
            .collect();

        let active = self.active_provider_name.clone();

        // Active provider first (preserves its original kind: complete or incomplete).
        if let Some(active_name) = active.as_deref()
            && let Some(entry) = self.archive.find(active_name)
        {
            if entry.has_api_key() {
                items.push(ItemKind::CompleteSaved(entry.clone()));
            } else {
                items.push(ItemKind::IncompleteSaved(entry.clone()));
            }
        }

        // Archive providers in file order (skip active, already added above).
        for entry in self.archive.providers.iter() {
            if Some(entry.name.as_str()) == active.as_deref() {
                continue;
            }
            if entry.has_api_key() {
                items.push(ItemKind::CompleteSaved(entry.clone()));
            } else {
                items.push(ItemKind::IncompleteSaved(entry.clone()));
            }
        }

        // Templates in returned order (skip those already in archive, skip active).
        for t in self.templates() {
            if HIDDEN_BUILTINS.contains(&t.name.as_str()) {
                continue;
            }
            if archive_names.contains(&t.name) {
                continue;
            }
            if Some(t.name.as_str()) == active.as_deref() {
                continue;
            }
            items.push(ItemKind::Template(t.clone()));
        }

        // Filter
        if !self.filter.is_empty() {
            let filter = self.filter.to_lowercase();
            items.retain(|item| item.display_name().to_lowercase().contains(&filter));
        }

        items
    }

    fn snap_focus_to_active(&mut self) {
        let items = self.build_items();
        if let Some(active_name) = self.active_provider_name.as_deref()
            && let Some(idx) = items.iter().position(|i| matches!(
                i,
                ItemKind::CompleteSaved(e) | ItemKind::IncompleteSaved(e) if e.name == active_name
            ))
        {
            self.selected_idx = idx;
            self.scroll_offset = 0;
        } else if self.selected_idx >= items.len() && !items.is_empty() {
            self.selected_idx = items.len() - 1;
            self.scroll_offset = 0;
        }
    }

    fn move_focus(&mut self, delta: i32) {
        let items = self.build_items();
        if items.is_empty() {
            return;
        }
        let len = items.len() as i32;
        let next = (self.selected_idx as i32 + delta).rem_euclid(len) as usize;
        self.selected_idx = next;

        // Keep a lightweight anchor for render-time dynamic viewport adjustment.
        if next < self.scroll_offset {
            self.scroll_offset = next;
        }
    }

    /// Enter key: proceed to next step if provider has existing key, otherwise open input.
    fn confirm(&mut self) {
        if let Some(name) = self.delete_confirm.take() {
            self.result = Some(SelectResult::DeleteRequested(name));
            self.is_done = true;
            return;
        }

        // If in inline input mode with content: save key and close input (stay on picker).
        if let InlineInputState::InlineInput {
            provider_name,
            buffer,
            existing_key: _,
        } = &self.inline_input_state
        {
            if !buffer.is_empty() {
                let name = provider_name.clone();
                let key = buffer.clone();
                self.result = Some(SelectResult::SaveKey { name, api_key: key });
                // Close inline input, stay on picker (is_done = false).
                self.inline_input_state = InlineInputState::ListNavigation;
            }
            return;
        }

        // List navigation mode: check selected provider.
        let items = self.build_items();
        let Some(item) = items.get(self.selected_idx) else {
            return;
        };

        match item {
            ItemKind::AddCustom => {
                self.result = Some(SelectResult::OpenCustomProvider);
            }
            ItemKind::CompleteSaved(entry) => {
                // Provider has api_key: proceed to next step and switch to this provider.
                self.result = Some(SelectResult::ProceedToNext {
                    name: entry.name.clone(),
                });
                self.is_done = true;
            }
            ItemKind::IncompleteSaved(_) | ItemKind::Template(_) => {
                // Provider needs api_key: open inline input.
                let name = match item {
                    ItemKind::Template(t) => t.name.clone(),
                    ItemKind::IncompleteSaved(e) => e.name.clone(),
                    _ => String::new(),
                };
                self.inline_input_state = InlineInputState::InlineInput {
                    provider_name: name,
                    buffer: String::new(),
                    existing_key: String::new(),
                };
            }
        }
    }

    /// Space key: toggle inline input (open/close).
    /// - In inline input mode: close and return to list.
    /// - In list navigation: open inline input, prefill with existing key if available.
    fn toggle_inline_input(&mut self) {
        // If in inline input mode: close it.
        if let InlineInputState::InlineInput { .. } = &self.inline_input_state {
            self.inline_input_state = InlineInputState::ListNavigation;
            return;
        }

        // List navigation: open inline input for selected provider.
        let items = self.build_items();
        let Some(item) = items.get(self.selected_idx) else {
            return;
        };

        let (name, existing_key) = match item {
            ItemKind::AddCustom => {
                self.result = Some(SelectResult::OpenCustomProvider);
                return;
            }
            ItemKind::Template(t) => (t.name.clone(), String::new()),
            ItemKind::IncompleteSaved(e) => (e.name.clone(), self.entry_api_key(e)),
            ItemKind::CompleteSaved(e) => (e.name.clone(), self.entry_api_key(e)),
        };

        // Prefill the buffer with the existing key so the user sees it's there
        // and can edit in place. The display shows a length-preserving mask.
        self.inline_input_state = InlineInputState::InlineInput {
            provider_name: name,
            buffer: existing_key.clone(),
            existing_key,
        };
    }

    fn entry_api_key(&self, entry: &ProviderEntry) -> String {
        entry.get_api_key(&self.agere_home)
    }
}

impl StepStateProvider for ProviderSelect {
    fn get_step_state(&self) -> StepState {
        if self.is_done || self.result.is_some() {
            StepState::Complete
        } else {
            StepState::InProgress
        }
    }
}

impl KeyboardHandler for ProviderSelect {
    fn handle_key_event(&mut self, key_event: KeyEvent) {
        if !matches!(key_event.kind, KeyEventKind::Press | KeyEventKind::Repeat) {
            return;
        }

        // Handle inline input mode first.
        if let InlineInputState::InlineInput { buffer, .. } = &mut self.inline_input_state {
            match key_event.code {
                KeyCode::Enter => {
                    if !buffer.is_empty() {
                        self.confirm();
                    }
                }
                KeyCode::Esc => {
                    self.inline_input_state = InlineInputState::ListNavigation;
                }
                KeyCode::Backspace => {
                    buffer.pop();
                }
                KeyCode::Char(c) => {
                    buffer.push(c);
                }
                _ => {}
            }
            return;
        }

        // Handle delete confirm dialog.
        if let Some(ref name) = self.delete_confirm {
            let name = name.clone();
            match key_event.code {
                KeyCode::Char('y') | KeyCode::Char('Y') | KeyCode::Enter => {
                    self.delete_confirm = None;
                    self.result = Some(SelectResult::DeleteRequested(name));
                }
                _ => {
                    self.delete_confirm = None;
                }
            }
            return;
        }

        // List navigation mode.
        let ctrl = key_event.modifiers.contains(KeyModifiers::CONTROL);

        match (key_event.code, ctrl) {
            (KeyCode::Esc, _) => {
                self.filter.clear();
            }
            (KeyCode::Up, _) => {
                // Close inline input when navigating away.
                if matches!(
                    self.inline_input_state,
                    InlineInputState::InlineInput { .. }
                ) {
                    self.inline_input_state = InlineInputState::ListNavigation;
                }
                self.move_focus(-1);
            }
            (KeyCode::Down, _) => {
                // Close inline input when navigating away.
                if matches!(
                    self.inline_input_state,
                    InlineInputState::InlineInput { .. }
                ) {
                    self.inline_input_state = InlineInputState::ListNavigation;
                }
                self.move_focus(1);
            }
            (KeyCode::PageUp, _) => self.move_focus(-5),
            (KeyCode::PageDown, _) => self.move_focus(5),
            (KeyCode::Home, _) => {
                self.selected_idx = 0;
                self.scroll_offset = 0;
            }
            (KeyCode::End, _) => {
                let items = self.build_items();
                self.selected_idx = items.len().saturating_sub(1);
                self.scroll_offset = self.selected_idx;
            }
            (KeyCode::Enter, _) => self.confirm(),
            (KeyCode::Backspace, _) => {
                self.filter.pop();
            }
            (KeyCode::Char(' '), false) => self.toggle_inline_input(),
            (KeyCode::Char('d'), true) => {
                // Ctrl+'d': trigger delete confirmation for custom providers.
                let items = self.build_items();
                if let Some(item) = items.get(self.selected_idx)
                    && item.is_custom()
                {
                    self.delete_confirm = Some(item.display_name().to_string());
                }
            }
            (KeyCode::Char(c), false) => {
                // Any other character input goes to filter.
                self.filter.push(c);
                self.selected_idx = 0;
            }
            _ => {}
        }
    }

    fn handle_paste(&mut self, pasted: String) {
        if pasted.is_empty() {
            return;
        }

        match &mut self.inline_input_state {
            InlineInputState::ListNavigation => {
                self.filter.push_str(&pasted);
                self.selected_idx = 0;
                self.scroll_offset = 0;
            }
            InlineInputState::InlineInput { buffer, .. } => {
                buffer.push_str(&pasted);
            }
        }
    }
}

impl WidgetRef for &ProviderSelect {
    fn render_ref(&self, area: Rect, buf: &mut Buffer) {
        Clear.render(area, buf);

        let content_area = self.render_outer_card(area, buf);
        let bottom = content_area.y.saturating_add(content_area.height);
        let inner_x = content_area.x;
        let inner_w = content_area.width;
        let line_area = |y: u16| Rect {
            x: inner_x,
            y,
            width: inner_w,
            height: 1,
        };

        let mut y = content_area.y;

        // Banner: title + tagline at the top of the unified welcome card.
        if self.show_welcome {
            y = welcome_frame::render_header(y, bottom);
        }

        // Card title + thin separator.
        if y < bottom {
            Line::from(vec![
                Span::from(format!(
                    "Discover providers ({}/{})",
                    self.selected_idx
                        .saturating_add(1)
                        .min(self.build_items().len().max(1)),
                    self.build_items().len()
                ))
                .bold(),
                "  ".into(),
                "including custom providers".dim(),
            ])
            .render_ref(line_area(y), buf);
            y = y.saturating_add(1);
        }
        if y < bottom {
            let separator: String = "─".repeat(inner_w.max(1) as usize);
            Line::from(separator.dim()).render_ref(line_area(y), buf);
            y = y.saturating_add(1);
        }

        // Header status hint (template loading / fallback notice).
        match &self.remote_state {
            TemplateLoadState::Loading if y < bottom => {
                Line::from(vec![
                    " ".into(),
                    "◌".cyan(),
                    "  ".into(),
                    "Loading providers from openagere.com…".dim().italic(),
                ])
                .render_ref(line_area(y), buf);
                y = y.saturating_add(1);
            }
            TemplateLoadState::Failed(msg) if y < bottom => {
                Line::from(vec![
                    " ".into(),
                    "⚠".red(),
                    "  ".into(),
                    Span::from(format!("remote unavailable ({msg}); showing offline list")).dim(),
                ])
                .render_ref(line_area(y), buf);
                y = y.saturating_add(1);
            }
            _ => {}
        }
        if y < bottom {
            y = y.saturating_add(1);
        }

        // Search input box at top; the terminal cursor is positioned by the owner.
        if y < bottom {
            y = self.render_search_box(line_area, y, bottom, buf, inner_w);
        }
        if y < bottom {
            y = y.saturating_add(1);
        }

        let items = self.build_items();
        let key_editor_area = self.key_editor_area(content_area, bottom);
        let list_bottom = key_editor_area
            .map(|area| area.y.saturating_sub(1))
            .unwrap_or(bottom);
        let visible_rows = ProviderSelect::visible_provider_rows(y, list_bottom);
        let scroll_offset = self.effective_scroll_offset(items.len(), visible_rows);

        if items.is_empty() {
            if y < list_bottom {
                Line::from(format!("  No match for \"{filter}\"", filter = self.filter).dim())
                    .render_ref(line_area(y), buf);
            }
        } else {
            // Apply render-time viewport height so the list expands with the terminal.
            let visible_items: Vec<_> = items
                .iter()
                .enumerate()
                .skip(scroll_offset)
                .take(visible_rows)
                .collect();

            for &(idx, item) in visible_items.iter() {
                if y >= list_bottom {
                    break;
                }
                y = self.render_item_row(line_area, y, list_bottom, buf, idx, item);
            }

            if scroll_offset + visible_items.len() < items.len() && y < list_bottom {
                Line::from("  ↓ more below".dim()).render_ref(line_area(y), buf);
            }
        }

        if let Some(area) = key_editor_area
            && let InlineInputState::InlineInput {
                provider_name,
                buffer,
                existing_key,
            } = &self.inline_input_state
        {
            self.render_key_editor(area, buf, provider_name, buffer, existing_key);
        }

        if let Some(name) = &self.delete_confirm
            && y < bottom
        {
            y = y.saturating_add(1);
            if y < bottom {
                let mut spans = vec![
                    "  ".into(),
                    "⚠".red().bold(),
                    "  ".into(),
                    Span::from(format!("Delete '{name}'? ")).red(),
                ];
                spans.extend(
                    provider_ui::hint_spans_with_alternates(&[
                        (&[KeyCode::Enter, KeyCode::Char('y')], "delete"),
                        (&[KeyCode::Esc, KeyCode::Char('n')], "keep"),
                    ])
                    .into_iter()
                    .skip(1),
                );
                Line::from(spans).render_ref(line_area(y), buf);
            }
        }
    }
}

impl ProviderSelect {
    pub(crate) fn cursor_pos(&self, area: Rect) -> Option<(u16, u16)> {
        let content_area = self.card_inner_area(area);
        let bottom = content_area.y.saturating_add(content_area.height);
        let inner_x = content_area.x;
        let inner_w = content_area.width;
        let mut y = content_area.y;

        if self.show_welcome {
            y = y.saturating_add(welcome_frame::HEADER_HEIGHT);
        }
        y = y.saturating_add(2);
        if matches!(
            self.remote_state,
            TemplateLoadState::Loading | TemplateLoadState::Failed(_)
        ) {
            y = y.saturating_add(1);
        }
        y = y.saturating_add(2);

        if matches!(self.inline_input_state, InlineInputState::ListNavigation) {
            if y >= bottom {
                return None;
            }
            let prefix_width = "│ ⌕ ".width() as u16;
            let filter_width = self.filter.width() as u16;
            let editable_width = inner_w.saturating_sub(prefix_width).saturating_sub(1);
            return Some((
                inner_x
                    .saturating_add(prefix_width)
                    .saturating_add(filter_width.min(editable_width.saturating_sub(1)))
                    .min(inner_x.saturating_add(inner_w.saturating_sub(2))),
                y,
            ));
        }

        if let InlineInputState::InlineInput { buffer, .. } = &self.inline_input_state
            && let Some(editor_area) = self.key_editor_area(content_area, bottom)
        {
            let text_area = self.key_editor_text_area(editor_area)?;
            let display_value = if buffer.is_empty() {
                String::new()
            } else {
                inline_key_display_value(buffer)
            };
            return provider_ui::cursor_pos_for_wrapped_text(
                &display_value,
                display_value.chars().count(),
                text_area,
            );
        }
        None
    }

    /// Return key hint spans for the onboarding footer.
    pub(crate) fn footer_hints(&self) -> Vec<Span<'_>> {
        if let InlineInputState::InlineInput { buffer, .. } = &self.inline_input_state {
            if !buffer.is_empty() {
                return provider_ui::hint_spans(&[
                    (KeyCode::Enter, "save"),
                    (KeyCode::Esc, "cancel"),
                ]);
            } else {
                return provider_ui::hint_spans(&[(KeyCode::Esc, "cancel")]);
            }
        }

        let items = self.build_items();
        let has_key = items
            .get(self.selected_idx)
            .map(|item| matches!(item, ItemKind::CompleteSaved(_)))
            .unwrap_or(false);

        if has_key {
            provider_ui::hint_spans_with_alternates(&[
                (&[KeyCode::Up, KeyCode::Down], "navigate"),
                (&[KeyCode::Enter], "next"),
                (&[KeyCode::Char(' ')], "edit key"),
            ])
        } else {
            provider_ui::hint_spans_with_alternates(&[
                (&[KeyCode::Up, KeyCode::Down], "navigate"),
                (&[KeyCode::Enter, KeyCode::Char(' ')], "set key"),
            ])
        }
    }

    fn card_inner_area(&self, area: Rect) -> Rect {
        if !self.show_welcome {
            return Rect {
                x: area.x.saturating_add(2),
                y: area.y,
                width: area.width.saturating_sub(4),
                height: area.height,
            };
        }

        Rect {
            ..welcome_frame::inner_area(area)
        }
    }

    fn render_outer_card(&self, area: Rect, buf: &mut Buffer) -> Rect {
        if !self.show_welcome {
            return self.card_inner_area(area);
        }

        welcome_frame::render(area, buf)
    }

    fn visible_provider_rows(start_y: u16, list_bottom: u16) -> usize {
        list_bottom
            .saturating_sub(start_y)
            .saturating_div(PROVIDER_ROW_HEIGHT)
            .max(1) as usize
    }

    fn effective_scroll_offset(&self, item_count: usize, visible_rows: usize) -> usize {
        if item_count == 0 {
            return 0;
        }
        let visible_rows = visible_rows.max(1).min(item_count);
        let max_offset = item_count.saturating_sub(visible_rows);
        let mut offset = self.scroll_offset.min(max_offset);
        if self.selected_idx < offset {
            offset = self.selected_idx;
        } else if self.selected_idx >= offset + visible_rows {
            offset = self.selected_idx.saturating_sub(visible_rows - 1);
        }
        offset.min(max_offset)
    }

    fn render_search_box(
        &self,
        line_area: impl Fn(u16) -> Rect,
        start_y: u16,
        bottom: u16,
        buf: &mut Buffer,
        inner_w: u16,
    ) -> u16 {
        let mut y = start_y;
        let width = inner_w.max(4) as usize;
        let top = format!("╭{}╮", "─".repeat(width.saturating_sub(2)));
        let bottom_border = format!("╰{}╯", "─".repeat(width.saturating_sub(2)));
        let query = if self.filter.is_empty() {
            "Search…".dim()
        } else {
            let query_width = width
                .saturating_sub("│ ⌕ ".width())
                .saturating_sub("│".width());
            let (query, _, _) = take_prefix_by_width(&self.filter, query_width);
            Span::from(query).cyan()
        };

        if y < bottom {
            Line::from(top.dim()).render_ref(line_area(y), buf);
            y = y.saturating_add(1);
        }
        if y < bottom {
            let prefix_width = "│ ⌕ ".width();
            let query_width = if self.filter.is_empty() {
                "Search…".width()
            } else {
                query.content.width()
            };
            let right_border_width = "│".width();
            let trailing = width
                .saturating_sub(prefix_width)
                .saturating_sub(query_width)
                .saturating_sub(right_border_width);
            Line::from(vec![
                "│".dim(),
                " ⌕ ".cyan(),
                query,
                " ".repeat(trailing).into(),
                "│".dim(),
            ])
            .render_ref(line_area(y), buf);
            y = y.saturating_add(1);
        }
        if y < bottom {
            Line::from(bottom_border.dim()).render_ref(line_area(y), buf);
            y = y.saturating_add(1);
        }

        y
    }

    fn render_item_row(
        &self,
        line_area: impl Fn(u16) -> Rect,
        start_y: u16,
        bottom: u16,
        buf: &mut Buffer,
        idx: usize,
        item: &ItemKind,
    ) -> u16 {
        let mut y = start_y;
        let selected = idx == self.selected_idx;
        let is_active = matches!(
            item,
            ItemKind::CompleteSaved(e) | ItemKind::IncompleteSaved(e)
            if Some(e.name.as_str()) == self.active_provider_name.as_deref()
        );

        let cursor: Span = if selected {
            Span::from("❯").cyan().bold()
        } else {
            Span::from(" ")
        };

        let status_dot: Span = match item {
            ItemKind::AddCustom => Span::from("+").cyan().bold(),
            _ if is_active => Span::from("●").green().bold(),
            ItemKind::CompleteSaved(_) => Span::from("●").green(),
            ItemKind::IncompleteSaved(_) => Span::from("○").red(),
            ItemKind::Template(_) => Span::from("·").dim(),
        };

        let name_span = match item {
            ItemKind::AddCustom => Span::from(item.display_name().to_string()).cyan().bold(),
            _ if is_active => Span::from(item.display_name().to_string()).green().bold(),
            ItemKind::CompleteSaved(_) => Span::from(item.display_name().to_string()).green(),
            ItemKind::IncompleteSaved(_) => Span::from(item.display_name().to_string()).red(),
            ItemKind::Template(_) => Span::from(item.display_name().to_string()),
        };

        let source: Span<'static> = match item {
            ItemKind::AddCustom => "custom provider".cyan().dim(),
            _ if is_active => Span::from("active").green().dim(),
            ItemKind::CompleteSaved(_) => "saved".green().dim(),
            ItemKind::IncompleteSaved(_) => "missing key".red().dim(),
            ItemKind::Template(_) => "provider template".dim(),
        };

        let detail = match item {
            ItemKind::AddCustom => "Create a provider from scratch".to_string(),
            _ => item.base_url().to_string(),
        };

        if y < bottom {
            Line::from(vec![
                "  ".into(),
                cursor,
                " ".into(),
                status_dot,
                " ".into(),
                name_span,
                " · ".dim(),
                source,
            ])
            .render_ref(line_area(y), buf);
            y = y.saturating_add(1);
        }
        if y < bottom {
            Line::from(vec!["      ".into(), Span::from(detail).dim()])
                .render_ref(line_area(y), buf);
            y = y.saturating_add(1);
        }

        y
    }

    fn key_editor_area(&self, content_area: Rect, bottom: u16) -> Option<Rect> {
        if matches!(self.inline_input_state, InlineInputState::ListNavigation)
            || content_area.width < 8
            || content_area.height < KEY_EDITOR_HEIGHT
        {
            return None;
        }

        Some(Rect {
            x: content_area.x,
            y: bottom.saturating_sub(KEY_EDITOR_HEIGHT),
            width: content_area.width,
            height: KEY_EDITOR_HEIGHT,
        })
    }

    fn key_editor_text_area(&self, area: Rect) -> Option<Rect> {
        if area.width <= 11 || area.height < KEY_EDITOR_HEIGHT {
            return None;
        }
        Some(Rect {
            x: area.x.saturating_add(9),
            y: area.y.saturating_add(1),
            width: area.width.saturating_sub(10),
            height: 1,
        })
    }

    fn render_key_editor(
        &self,
        area: Rect,
        buf: &mut Buffer,
        provider_name: &str,
        buffer: &str,
        _existing_key: &str,
    ) {
        if area.width < 8 || area.height < KEY_EDITOR_HEIGHT {
            return;
        }

        let separator: String = "─".repeat(area.width.max(1) as usize);
        Line::from(separator.dim()).render_ref(Rect { height: 1, ..area }, buf);

        // The buffer may contain a prefilled existing key; always render the
        // masked edit buffer and never reveal the raw secret.
        let provider_suffix = format!("  {provider_name}");
        let value_width = area
            .width
            .saturating_sub("  key  › ".width() as u16)
            .saturating_sub(provider_suffix.width() as u16) as usize;
        let display_value = inline_key_display_value(buffer);
        let (display_value, _, _) = take_prefix_by_width(&display_value, value_width);
        let value = if buffer.is_empty() {
            Span::from(display_value).dim()
        } else {
            Span::from(display_value).cyan()
        };
        Line::from(vec![
            "  ".into(),
            "key".dim(),
            "  ".into(),
            "› ".bold(),
            value,
            Span::from(provider_suffix).dim(),
        ])
        .render_ref(
            Rect {
                y: area.y.saturating_add(1),
                height: 1,
                ..area
            },
            buf,
        );

        let hints = if buffer.is_empty() {
            provider_ui::hint_spans(&[(KeyCode::Esc, "cancel")])
        } else {
            provider_ui::hint_spans(&[(KeyCode::Enter, "save"), (KeyCode::Esc, "cancel")])
        };
        Line::from(hints).render_ref(
            Rect {
                y: area.y.saturating_add(2),
                height: 1,
                ..area
            },
            buf,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use agere_config::config_toml::ModelConfig;
    use agere_model_provider_info::WireApi;
    use insta::assert_snapshot;
    use pretty_assertions::assert_eq;
    use ratatui::buffer::Buffer;

    fn entry(name: &str, api_key: Option<&str>, is_custom: bool) -> ProviderEntry {
        ProviderEntry {
            name: name.to_string(),
            base_url: format!("https://api.{name}.test"),
            wire_api: WireApi::Chat,
            env_key: None,
            encrypted_api_key: api_key.map(str::to_string),
            is_custom,
            models: vec![ModelConfig {
                name: "default".to_string(),
                context_window: Some(128000),
                input_modalities: None,
            }],
        }
    }

    fn template(name: &str) -> ProviderTemplate {
        ProviderTemplate {
            name: name.to_string(),
            base_url: format!("https://api.{name}.test"),
            wire_api: WireApi::Chat,
            env_key: None,
            models: vec![ModelConfig {
                name: "default".to_string(),
                context_window: Some(128000),
                input_modalities: None,
            }],
        }
    }

    fn sample() -> ProviderSelect {
        let archive = ProviderToml {
            providers: vec![
                entry("openai", Some("sk-1"), false),
                entry("deepseek", None, false),
                entry("my-custom", Some("k"), true),
            ],
            warnings: Vec::new(),
        };
        let templates = vec![template("anthropic"), template("openai"), template("qwen")];
        ProviderSelect::new(
            archive,
            PathBuf::new(),
            templates,
            Some("openai".to_string()),
        )
    }

    fn render_snapshot(view: &ProviderSelect, width: u16, height: u16) -> String {
        let area = Rect::new(0, 0, width, height);
        let mut buf = Buffer::empty(area);
        view.render_ref(area, &mut buf);
        format!("{buf:?}")
    }

    #[test]
    fn add_custom_appears_first_and_active_provider_focused() {
        let s = sample();
        let items = s.build_items();
        assert!(matches!(items.first(), Some(ItemKind::AddCustom)));
        assert!(
            matches!(items.get(s.selected_idx), Some(ItemKind::CompleteSaved(e)) if e.name == "openai")
        );
    }

    #[test]
    fn enter_on_add_custom_opens_custom_provider() {
        let mut s = sample();
        s.selected_idx = 0;
        s.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        match s.take_result() {
            Some(SelectResult::OpenCustomProvider) => {}
            other => panic!("expected OpenCustomProvider, got {other:?}"),
        }
        assert_eq!(s.get_step_state(), StepState::InProgress);
    }

    #[test]
    fn space_on_add_custom_keeps_provider_select_in_progress() {
        let mut s = sample();
        s.selected_idx = 0;
        s.handle_key_event(KeyEvent::new(KeyCode::Char(' '), KeyModifiers::NONE));
        match s.take_result() {
            Some(SelectResult::OpenCustomProvider) => {}
            other => panic!("expected OpenCustomProvider, got {other:?}"),
        }
        assert_eq!(s.get_step_state(), StepState::InProgress);
    }

    #[test]
    fn welcome_layout_groups_brand_and_providers() {
        let mut s = sample();
        s.enable_welcome_banner();
        assert_snapshot!(render_snapshot(&s, 86, 20));
    }

    #[test]
    fn visible_provider_rows_scale_with_available_height() {
        assert_eq!(ProviderSelect::visible_provider_rows(7, 14), 3);
        assert_eq!(ProviderSelect::visible_provider_rows(7, 24), 8);

        let mut s = sample();
        s.selected_idx = 9;
        s.scroll_offset = 0;
        assert_eq!(s.effective_scroll_offset(12, 3), 7);
        assert_eq!(s.effective_scroll_offset(12, 8), 2);
    }

    #[test]
    fn inline_input_renders_guided_card() {
        let mut s = sample();
        s.inline_input_state = InlineInputState::InlineInput {
            provider_name: "deepseek".to_string(),
            buffer: String::new(),
            existing_key: String::new(),
        };
        loop {
            let items = s.build_items();
            let item = &items[s.selected_idx];
            if matches!(item, ItemKind::IncompleteSaved(e) if e.name == "deepseek") {
                break;
            }
            s.move_focus(1);
        }
        assert_snapshot!(render_snapshot(&s, 96, 18));
    }

    #[test]
    fn inline_input_uses_fixed_footer_editor_and_cursor() {
        let templates = vec![
            template("p0"),
            template("p1"),
            template("p2"),
            template("p3"),
            template("Z.AI"),
        ];
        let mut s = ProviderSelect::new(ProviderToml::default(), PathBuf::new(), templates, None);
        loop {
            let items = s.build_items();
            let item = &items[s.selected_idx];
            if matches!(item, ItemKind::Template(t) if t.name == "Z.AI") {
                break;
            }
            s.move_focus(1);
        }
        s.inline_input_state = InlineInputState::InlineInput {
            provider_name: "Z.AI".to_string(),
            buffer: String::new(),
            existing_key: String::new(),
        };

        let area = Rect::new(0, 0, 96, 18);
        let content_area = s.card_inner_area(area);
        let cursor = s.cursor_pos(area).unwrap();
        let editor_area = s
            .key_editor_area(content_area, content_area.bottom())
            .unwrap();
        let text_area = s.key_editor_text_area(editor_area).unwrap();

        assert_eq!(
            editor_area.y + editor_area.height,
            content_area.y + content_area.height
        );
        assert_eq!(cursor.1, text_area.y);
        assert!(cursor.0 >= text_area.x);
        assert!(cursor.0 < text_area.x + text_area.width);
    }

    #[test]
    fn chinese_inline_key_cursor_uses_masked_display_width() {
        let mut s = sample();
        s.inline_input_state = InlineInputState::InlineInput {
            provider_name: "deepseek".to_string(),
            buffer: "中文中文中文中文中文".to_string(),
            existing_key: String::new(),
        };

        let area = Rect::new(0, 0, 48, 16);
        let content_area = s.card_inner_area(area);
        let editor_area = s
            .key_editor_area(content_area, content_area.bottom())
            .unwrap();
        let text_area = s.key_editor_text_area(editor_area).unwrap();
        let cursor = s.cursor_pos(area).unwrap();
        let expected_x = text_area.x + mask_key_with_ends("中文中文中文中文中文").width() as u16;

        assert_eq!(cursor, (expected_x, text_area.y));
        assert!(cursor.0 < text_area.x + text_area.width);
    }

    #[test]
    fn templates_dedup_against_archive() {
        let s = sample();
        let items = s.build_items();
        let openai_count = items
            .iter()
            .filter(|i| match i {
                ItemKind::AddCustom => false,
                ItemKind::CompleteSaved(e) | ItemKind::IncompleteSaved(e) => e.name == "openai",
                ItemKind::Template(t) => t.name == "openai",
            })
            .count();
        assert_eq!(openai_count, 1);
    }

    #[test]
    fn amazon_bedrock_is_hidden() {
        let archive = ProviderToml::default();
        let templates = vec![template("amazon-bedrock"), template("openai")];
        let s = ProviderSelect::new(archive, PathBuf::new(), templates, None);
        let items = s.build_items();
        let has_bedrock = items
            .iter()
            .any(|i| matches!(i, ItemKind::Template(t) if t.name == "amazon-bedrock"));
        assert!(!has_bedrock);
    }

    #[test]
    fn filter_uses_substring_contains() {
        let mut s = sample();
        s.handle_key_event(KeyEvent::new(KeyCode::Char('a'), KeyModifiers::NONE));
        s.handle_key_event(KeyEvent::new(KeyCode::Char('n'), KeyModifiers::NONE));
        s.handle_key_event(KeyEvent::new(KeyCode::Char('t'), KeyModifiers::NONE));
        let items = s.build_items();
        assert!(
            items
                .iter()
                .any(|i| matches!(i, ItemKind::Template(t) if t.name == "anthropic"))
        );
    }

    #[test]
    fn chinese_filter_keeps_search_box_right_border_visible() {
        let mut s = sample();
        s.filter = "中文中文中文中文中文中文中文中文中文".to_string();

        let area = Rect::new(0, 0, 40, 16);
        let content_area = s.card_inner_area(area);
        let mut buf = Buffer::empty(area);
        (&s).render_ref(area, &mut buf);

        let search_row = content_area.y + 4;
        assert_eq!(buf[(content_area.right() - 1, search_row)].symbol(), "│");
        let cursor = s.cursor_pos(area).unwrap();
        assert!(cursor.0 < content_area.right() - 1);
    }

    #[test]
    fn paste_in_list_navigation_updates_filter() {
        let mut s = sample();
        s.handle_paste("deep".to_string());
        assert_eq!(s.filter, "deep");
        let items = s.build_items();
        assert_eq!(items.len(), 1);
        assert!(
            matches!(items.first(), Some(ItemKind::IncompleteSaved(e)) if e.name == "deepseek")
        );
    }

    #[test]
    fn paste_in_inline_input_updates_api_key_buffer() {
        let mut s = sample();
        s.inline_input_state = InlineInputState::InlineInput {
            provider_name: "deepseek".to_string(),
            buffer: String::new(),
            existing_key: String::new(),
        };
        s.handle_paste("sk-from-right-click".to_string());
        assert!(matches!(
            s.inline_input_state,
            InlineInputState::InlineInput { ref buffer, .. } if buffer == "sk-from-right-click"
        ));
    }

    #[test]
    fn d_char_goes_into_filter() {
        let mut s = sample();
        s.handle_key_event(KeyEvent::new(KeyCode::Char('d'), KeyModifiers::NONE));
        s.handle_key_event(KeyEvent::new(KeyCode::Char('e'), KeyModifiers::NONE));
        s.handle_key_event(KeyEvent::new(KeyCode::Char('e'), KeyModifiers::NONE));
        s.handle_key_event(KeyEvent::new(KeyCode::Char('p'), KeyModifiers::NONE));
        assert_eq!(s.filter, "deep");
        let items = s.build_items();
        assert!(
            items
                .iter()
                .any(|i| matches!(i, ItemKind::IncompleteSaved(e) if e.name == "deepseek"))
        );
    }

    #[test]
    fn enter_on_other_complete_emits_proceed() {
        let mut archive = ProviderToml::default();
        archive.providers.push(entry("openai", Some("sk-1"), false));
        archive
            .providers
            .push(entry("deepseek", Some("sk-2"), false));
        let mut s = ProviderSelect::new(
            archive,
            PathBuf::new(),
            Vec::new(),
            Some("openai".to_string()),
        );
        // selected_idx defaults to 0 = active openai; move to next which is deepseek (complete).
        s.move_focus(1);
        s.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        match s.take_result() {
            Some(SelectResult::ProceedToNext { name }) => assert_eq!(name, "deepseek"),
            other => panic!("expected ProceedToNext, got {other:?}"),
        }
    }

    #[test]
    fn enter_on_incomplete_enters_inline_mode() {
        let mut s = sample();
        // Move to the saved-incomplete row "deepseek".
        loop {
            let items = s.build_items();
            let item = &items[s.selected_idx];
            if matches!(item, ItemKind::IncompleteSaved(e) if e.name == "deepseek") {
                break;
            }
            s.move_focus(1);
        }
        s.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        // Should enter inline input mode, not emit a result yet.
        assert!(matches!(
            s.inline_input_state,
            InlineInputState::InlineInput { ref provider_name, .. } if provider_name == "deepseek"
        ));
        assert!(s.take_result().is_none());
    }

    #[test]
    fn enter_on_template_enters_inline_mode() {
        let mut s = sample();
        // Move to a template row "anthropic".
        loop {
            let items = s.build_items();
            let item = &items[s.selected_idx];
            if matches!(item, ItemKind::Template(t) if t.name == "anthropic") {
                break;
            }
            s.move_focus(1);
        }
        s.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        assert!(matches!(
            s.inline_input_state,
            InlineInputState::InlineInput { ref provider_name, .. } if provider_name == "anthropic"
        ));
    }

    #[test]
    fn inline_input_submit_emits_save_key() {
        let mut s = sample();
        // Enter inline mode for deepseek.
        s.inline_input_state = InlineInputState::InlineInput {
            provider_name: "deepseek".to_string(),
            buffer: "sk-test-key".to_string(),
            existing_key: String::new(),
        };
        s.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        match s.take_result() {
            Some(SelectResult::SaveKey { name, api_key }) => {
                assert_eq!(name, "deepseek");
                assert_eq!(api_key, "sk-test-key");
            }
            other => panic!("expected SaveKey, got {other:?}"),
        }
        // SaveKey does NOT set is_done — picker stays open.
        assert!(!s.is_done);
    }

    #[test]
    fn empty_inline_input_enter_keeps_editing() {
        let mut s = sample();
        s.inline_input_state = InlineInputState::InlineInput {
            provider_name: "deepseek".to_string(),
            buffer: String::new(),
            existing_key: String::new(),
        };
        s.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        assert!(matches!(
            s.inline_input_state,
            InlineInputState::InlineInput { ref provider_name, ref buffer, .. }
                if provider_name == "deepseek" && buffer.is_empty()
        ));
        assert!(s.take_result().is_none());
    }

    #[test]
    fn ctrl_d_only_targets_custom_providers() {
        let mut s = sample();
        // Move to "my-custom"
        loop {
            let items = s.build_items();
            let item = &items[s.selected_idx];
            if matches!(item, ItemKind::CompleteSaved(e) if e.name == "my-custom") {
                break;
            }
            s.move_focus(1);
        }
        s.handle_key_event(KeyEvent::new(KeyCode::Char('d'), KeyModifiers::CONTROL));
        assert_eq!(s.delete_confirm.as_deref(), Some("my-custom"));
        s.handle_key_event(KeyEvent::new(KeyCode::Char('y'), KeyModifiers::NONE));
        match s.take_result() {
            Some(SelectResult::DeleteRequested(name)) => assert_eq!(name, "my-custom"),
            other => panic!("expected DeleteRequested, got {other:?}"),
        }
    }

    #[test]
    fn esc_clears_filter_and_never_cancels() {
        let mut s = sample();
        // Filter is not empty, so ESC just clears the filter.
        s.handle_key_event(KeyEvent::new(KeyCode::Char('a'), KeyModifiers::NONE));
        s.handle_key_event(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
        assert!(s.filter.is_empty());
        assert!(s.take_result().is_none());

        // Second ESC with empty filter but show_welcome = false still does not emit Cancel.
        s.handle_key_event(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
        assert!(s.filter.is_empty());
        assert!(s.take_result().is_none());
    }

    #[test]
    fn esc_clears_filter_on_welcome_screen() {
        let mut s = sample();
        s.enable_welcome_banner();
        s.filter = "open".to_string();
        s.handle_key_event(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
        assert!(s.filter.is_empty());
        assert!(s.take_result().is_none());
        assert!(!s.is_done);
    }
}
