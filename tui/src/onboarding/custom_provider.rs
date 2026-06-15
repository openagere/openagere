//! Custom provider configuration page. All fields inline-editable.
//! Navigate with up/down, type directly, left/right cycles WireApi.
//! Enter submits when all fields are filled. ESC returns to provider selection.

use agere_config::config_toml::ModelConfig;
use agere_model_provider_info::WireApi;
use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyEventKind;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::prelude::Widget;
use ratatui::style::Stylize;
use ratatui::text::Line;
use ratatui::text::Span;
use ratatui::widgets::Clear;
use ratatui::widgets::WidgetRef;
use std::cell::Cell;

use crate::onboarding::onboarding_screen::KeyboardHandler;
use crate::onboarding::onboarding_screen::StepState;
use crate::onboarding::onboarding_screen::StepStateProvider;
use crate::onboarding::welcome_frame;
use crate::provider_ui;
use crate::provider_ui::ProviderFieldKind;
use crate::provider_ui::ProviderFormKind;

const FIELD_LABEL_WIDTH: usize = 10;
const FIELD_STATUS_WIDTH: usize = 8;
const FIELD_VALUE_PREFIX_WIDTH: usize = 2 + FIELD_LABEL_WIDTH + 2 + FIELD_STATUS_WIDTH + 2;

#[derive(Clone, Debug)]
pub(crate) enum CustomProviderResult {
    Back,
    Saved {
        name: String,
        base_url: String,
        wire_api: WireApi,
        api_key: String,
        models: Vec<ModelConfig>,
    },
}

impl StepStateProvider for CustomProvider {
    fn get_step_state(&self) -> StepState {
        if self.result.is_some() {
            StepState::Complete
        } else {
            StepState::InProgress
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::KeyModifiers;
    use insta::assert_snapshot;
    use pretty_assertions::assert_eq;
    use ratatui::buffer::Buffer;

    fn make() -> CustomProvider {
        CustomProvider::new(vec!["existing".to_string()])
    }

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    fn render_snapshot(view: &CustomProvider, width: u16, height: u16) -> String {
        let area = Rect::new(0, 0, width, height);
        let mut buf = Buffer::empty(area);
        view.render_ref(area, &mut buf);
        format!("{buf:?}")
    }

    #[test]
    fn default_values() {
        let cp = make();
        assert!(cp.name.value.is_empty());
        assert!(cp.base_url.value.is_empty());
        assert_eq!(cp.wire_api, WireApi::Responses);
        assert!(cp.api_key.value.is_empty());
        assert!(cp.models.value.is_empty());
        assert_eq!(cp.focused, 0);
    }

    #[test]
    fn up_down_navigation() {
        let mut cp = make();
        cp.handle_key_event(key(KeyCode::Down));
        assert_eq!(cp.focused, 1);
        cp.handle_key_event(key(KeyCode::Up));
        assert_eq!(cp.focused, 0);
        cp.handle_key_event(key(KeyCode::Up));
        assert_eq!(cp.focused, 4);
    }

    #[test]
    fn esc_returns_to_provider_select() {
        let mut cp = make();
        cp.name.value = "test".into();
        cp.handle_key_event(key(KeyCode::Esc));
        assert!(matches!(cp.take_result(), Some(CustomProviderResult::Back)));
        assert_eq!(cp.name.value, "test");
    }

    #[test]
    fn successful_submit() {
        let mut cp = make();
        cp.name.value = "my-provider".into();
        cp.base_url.value = "https://api.example.com/v1".into();
        cp.api_key.value = "sk-test-123".into();
        cp.models.value = "gpt-4o[200k]".into();
        cp.wire_api = WireApi::Chat;
        cp.handle_key_event(key(KeyCode::Enter));
        match cp.take_result() {
            Some(CustomProviderResult::Saved {
                name,
                base_url,
                wire_api,
                models,
                ..
            }) => {
                assert_eq!(name, "my-provider");
                assert_eq!(base_url, "https://api.example.com/v1");
                assert_eq!(wire_api, WireApi::Chat);
                assert_eq!(models.len(), 1);
                assert_eq!(models[0].name, "gpt-4o");
            }
            other => panic!("expected Saved, got {other:?}"),
        }
    }

    #[test]
    fn submit_rejects_duplicate_name() {
        let mut cp = make();
        cp.name.value = "existing".into();
        cp.base_url.value = "https://api.example.com/v1".into();
        cp.api_key.value = "sk-test".into();
        cp.models.value = "m1".into();
        cp.handle_key_event(key(KeyCode::Enter));
        assert!(cp.take_result().is_none());
        assert!(cp.error.is_some());
    }

    #[test]
    fn submit_rejects_empty_fields() {
        let mut cp = make();
        cp.handle_key_event(key(KeyCode::Enter));
        assert!(cp.take_result().is_none());
        assert!(cp.error.is_some());
    }

    #[test]
    fn submit_rejects_invalid_base_url() {
        let mut cp = make();
        cp.name.value = "test".into();
        cp.base_url.value = "ftp://bad.com".into();
        cp.api_key.value = "sk-test".into();
        cp.models.value = "m1".into();
        cp.handle_key_event(key(KeyCode::Enter));
        assert!(cp.take_result().is_none());
    }

    #[test]
    fn is_submit_ready_checks_all_fields() {
        let mut cp = make();
        assert!(!cp.is_submit_ready());
        cp.name.value = "test".into();
        assert!(!cp.is_submit_ready());
        cp.base_url.value = "https://api.test.com/v1".into();
        assert!(!cp.is_submit_ready());
        cp.api_key.value = "sk-test".into();
        assert!(!cp.is_submit_ready());
        cp.models.value = "m1".into();
        assert!(cp.is_submit_ready());
    }

    #[test]
    fn wire_api_cycles_left_right() {
        let mut cp = make();
        cp.focused = 2;
        assert_eq!(cp.wire_api, WireApi::Responses);
        cp.handle_key_event(key(KeyCode::Right));
        assert_eq!(cp.wire_api, WireApi::Chat);
        cp.handle_key_event(key(KeyCode::Right));
        assert_eq!(cp.wire_api, WireApi::Anthropic);
        cp.handle_key_event(key(KeyCode::Right));
        assert_eq!(cp.wire_api, WireApi::Responses);
        cp.handle_key_event(key(KeyCode::Left));
        assert_eq!(cp.wire_api, WireApi::Anthropic);
    }

    #[test]
    fn text_editing_insert_delete() {
        let mut cp = make();
        cp.focused = 0;
        cp.handle_key_event(key(KeyCode::Char('a')));
        cp.handle_key_event(key(KeyCode::Char('b')));
        assert_eq!(cp.name.value, "ab");
        cp.handle_key_event(key(KeyCode::Backspace));
        assert_eq!(cp.name.value, "a");
    }

    #[test]
    fn text_cursor_movement() {
        let mut cp = make();
        cp.name.value = "hello".into();
        cp.name.cursor = 2;
        cp.focused = 0;
        cp.handle_key_event(key(KeyCode::Left));
        assert_eq!(cp.name.cursor, 1);
        cp.handle_key_event(key(KeyCode::Home));
        assert_eq!(cp.name.cursor, 0);
        cp.handle_key_event(key(KeyCode::End));
        assert_eq!(cp.name.cursor, 5);
    }

    #[test]
    fn up_down_moves_text_cursor_before_switching_fields() {
        let mut cp = make();
        cp.models.value = "abcdefghi".into();
        cp.models.cursor = 7;
        cp.focused = CustomProvider::fields()
            .iter()
            .position(|field| *field == ProviderFieldKind::Models)
            .expect("models field");
        cp.set_field_text_width(ProviderFieldKind::Models, 3);

        cp.handle_key_event(key(KeyCode::Up));
        assert_eq!(cp.focused_field(), ProviderFieldKind::Models);
        assert_eq!(cp.models.cursor, 4);

        cp.handle_key_event(key(KeyCode::Down));
        assert_eq!(cp.focused_field(), ProviderFieldKind::Models);
        assert_eq!(cp.models.cursor, 7);

        cp.handle_key_event(key(KeyCode::Down));
        assert_eq!(cp.focused_field(), ProviderFieldKind::Name);
    }

    #[test]
    fn paste_insertion() {
        let mut cp = make();
        cp.focused = 0;
        cp.handle_paste("hello".into());
        assert_eq!(cp.name.value, "hello");
    }

    #[test]
    fn paste_ignored_for_wire_api() {
        let mut cp = make();
        cp.focused = 2;
        cp.wire_api = WireApi::Chat;
        cp.handle_paste("responses".into());
        assert_eq!(cp.wire_api, WireApi::Chat);
    }

    #[test]
    fn parse_models_mixed() {
        let models =
            provider_ui::parse_models("gpt-4o[200k],claude,deepseek[2M]").expect("valid models");
        assert_eq!(models.len(), 3);
        assert_eq!(models[0].context_window, Some(200_000));
        assert_eq!(models[1].context_window, None);
        assert_eq!(models[2].context_window, Some(2_000_000));
    }

    #[test]
    fn mask_key_short_long() {
        assert_eq!(provider_ui::mask_key("abc"), "•••");
        let m = provider_ui::mask_key("sk-test-key-12345678");
        assert!(m.starts_with("sk-t"));
        assert!(m.ends_with("5678"));
    }

    #[test]
    fn fields_keep_base_url_and_skip_env_key() {
        assert_eq!(
            CustomProvider::fields(),
            &[
                ProviderFieldKind::Name,
                ProviderFieldKind::BaseUrl,
                ProviderFieldKind::WireApi,
                ProviderFieldKind::ApiKey,
                ProviderFieldKind::Models,
            ]
        );
    }

    #[test]
    fn custom_provider_form_snapshot() {
        let mut cp = make();
        cp.name.value = "my-provider".to_string();
        cp.name.cursor = cp.name.value.chars().count();
        cp.base_url.value = "https://gateway.example.com/openai-compatible/v1".to_string();
        cp.api_key.value = "sk-custom-provider-example-key".to_string();
        cp.models.value = "custom-chat-model[256k],custom-reasoning-model[1m]".to_string();
        cp.focused = 1;

        assert_snapshot!(
            "custom_provider_form_snapshot",
            render_snapshot(&cp, 62, 20)
        );
    }
}

impl WidgetRef for &CustomProvider {
    fn render_ref(&self, area: Rect, buf: &mut Buffer) {
        Clear.render(area, buf);
        let content_area = welcome_frame::render(area, buf);
        let inner_x = content_area.x;
        let inner_w = content_area.width;
        let line_area = |y: u16| Rect {
            x: inner_x,
            y,
            width: inner_w,
            height: 1,
        };
        let mut y = content_area.y;
        let bottom = content_area.y.saturating_add(content_area.height);

        y = welcome_frame::render_header(y, bottom);
        if y < bottom {
            Line::from(vec![
                "Create custom provider".into(),
                "  ".into(),
                "Required fields first".dim(),
            ])
            .render_ref(line_area(y), buf);
            y = y.saturating_add(1);
        }
        if y < bottom {
            let separator: String = "─".repeat(inner_w.max(1) as usize);
            Line::from(separator.dim()).render_ref(line_area(y), buf);
            y = y.saturating_add(1);
        }
        if y < bottom {
            y = y.saturating_add(1);
        }
        for (i, &field) in CustomProvider::fields().iter().enumerate() {
            if y >= bottom {
                break;
            }
            self.render_field(line_area(y), buf, field, i, bottom);
            y = y.saturating_add(self.field_height(field, inner_w));
        }
        if y + 1 < bottom {
            y = y.saturating_add(1);
            self.render_focused_help(line_area(y), buf);
            y = y.saturating_add(1);
        }
        if let Some(err) = &self.error {
            if y + 1 < bottom {
                y = y.saturating_add(1);
            }
            if y < bottom {
                Line::from(vec!["  ✖  ".into(), Span::from(err.clone()).red()])
                    .render_ref(line_area(y), buf);
            }
        }
    }
}

impl CustomProvider {
    pub(crate) fn cursor_pos(&self, area: Rect) -> Option<(u16, u16)> {
        let field = self.focused_field();
        if field == ProviderFieldKind::WireApi {
            return None;
        }

        let content_area = welcome_frame::inner_area(area);
        let inner_x = content_area.x;
        let inner_w = content_area.width;
        let mut y = self.fields_start_y(content_area);
        for (idx, &candidate) in Self::fields().iter().enumerate() {
            if idx == self.focused {
                let value_x = inner_x.saturating_add(FIELD_VALUE_PREFIX_WIDTH as u16);
                let text_area = Rect {
                    x: value_x,
                    y,
                    width: inner_w
                        .saturating_sub(value_x.saturating_sub(inner_x))
                        .max(1),
                    height: self.field_height(candidate, inner_w),
                };
                let input_area = Rect {
                    x: text_area.x,
                    y: text_area.y,
                    width: text_area.width,
                    height: area
                        .y
                        .saturating_add(area.height)
                        .saturating_sub(text_area.y),
                };
                let state = self.text_state(field);
                self.set_field_text_width(field, input_area.width.max(1));
                return provider_ui::cursor_pos_for_wrapped_text(
                    &state.value,
                    state.cursor,
                    input_area,
                );
            }
            y = y.saturating_add(self.field_height(candidate, inner_w));
        }
        None
    }

    fn field_height(&self, field: ProviderFieldKind, width: u16) -> u16 {
        match field {
            ProviderFieldKind::WireApi => 1,
            _ => provider_ui::wrap_ranges(
                &self.field_display_value(field),
                self.field_value_width(width),
            )
            .len() as u16,
        }
    }

    /// Return key hint spans for the onboarding footer.
    pub(crate) fn footer_hints(&self) -> Vec<Span<'_>> {
        if self.is_submit_ready() {
            provider_ui::hint_spans_with_alternates(&[
                (&[KeyCode::Enter], "submit"),
                (&[KeyCode::Esc], "back"),
                (&[KeyCode::Up, KeyCode::Down], "navigate"),
            ])
        } else {
            provider_ui::hint_spans_with_alternates(&[
                (&[KeyCode::Enter], "submit (fill all fields)"),
                (&[KeyCode::Esc], "back"),
                (&[KeyCode::Up, KeyCode::Down], "navigate"),
            ])
        }
    }

    fn fields_start_y(&self, content_area: Rect) -> u16 {
        content_area
            .y
            .saturating_add(welcome_frame::HEADER_HEIGHT)
            .saturating_add(3)
    }

    fn render_field(
        &self,
        area: Rect,
        buf: &mut Buffer,
        field: ProviderFieldKind,
        idx: usize,
        bottom: u16,
    ) {
        let focused = idx == self.focused;
        let cursor = if focused {
            Span::from("›").cyan().bold()
        } else {
            Span::from(" ")
        };
        let label = format!("{:<FIELD_LABEL_WIDTH$}", field.label());
        let status = self.field_status(field);
        let status_text = format!("{:<FIELD_STATUS_WIDTH$}", status.0);
        let status_span = if status.1 {
            Span::from(status_text).green()
        } else {
            Span::from(status_text).red()
        };
        // Use white for all field labels for better visibility.
        let label_span = Span::from(label);
        let mut spans = vec![
            cursor,
            " ".into(),
            label_span,
            "  ".into(),
            status_span,
            "  ".into(),
        ];
        match field {
            ProviderFieldKind::WireApi => {
                self.render_wire_api_value(&mut spans);
                Line::from(spans).render_ref(area, buf);
            }
            _ => self.render_text_value(spans, area, buf, field, focused, bottom),
        }
    }

    fn render_text_value(
        &self,
        first_line_spans: Vec<Span>,
        area: Rect,
        buf: &mut Buffer,
        field: ProviderFieldKind,
        focused: bool,
        bottom: u16,
    ) {
        let shown = self.field_display_value(field);
        let value_width = self.field_value_width(area.width);
        self.set_field_text_width(field, value_width);
        let ranges = provider_ui::wrap_ranges(&shown, value_width);
        for (line_idx, range) in ranges.iter().enumerate() {
            let y = area.y.saturating_add(line_idx as u16);
            if y >= bottom {
                break;
            }
            let mut spans = if line_idx == 0 {
                first_line_spans.clone()
            } else {
                vec![
                    " ".into(),
                    " ".into(),
                    Span::from(" ".repeat(FIELD_LABEL_WIDTH)),
                    "  ".into(),
                    Span::from(" ".repeat(FIELD_STATUS_WIDTH)),
                    "  ".into(),
                ]
            };
            let line_text = shown[range.clone()].to_string();
            let value_span: Span = if focused {
                Span::from(line_text).cyan()
            } else if self.text_state(field).value.is_empty() {
                Span::from(line_text).dim()
            } else {
                Span::from(line_text)
            };
            spans.push(value_span);
            Line::from(spans).render_ref(Rect { y, ..area }, buf);
        }
    }

    fn field_display_value(&self, field: ProviderFieldKind) -> String {
        let state = self.text_state(field);
        if state.value.is_empty() {
            field.placeholder().to_string()
        } else if matches!(field, ProviderFieldKind::ApiKey) && field != self.focused_field() {
            provider_ui::mask_key(&state.value)
        } else {
            state.value.clone()
        }
    }

    fn field_value_width(&self, width: u16) -> u16 {
        width.saturating_sub(FIELD_VALUE_PREFIX_WIDTH as u16).max(1)
    }

    fn render_wire_api_value(&self, spans: &mut Vec<Span>) {
        for (i, &api) in provider_ui::WIRE_APIS.iter().enumerate() {
            let name = provider_ui::wire_api_name(api);
            let indicator = if api == self.wire_api { "●" } else { "○" };
            if i > 0 {
                spans.push("  ".into());
            }
            let indicator_span = if api == self.wire_api {
                Span::from(indicator).cyan().bold()
            } else {
                Span::from(indicator).dim()
            };
            spans.push(indicator_span);
            spans.push(" ".into());
            let name_span = if api == self.wire_api {
                Span::from(name).cyan()
            } else {
                Span::from(name).dim()
            };
            spans.push(name_span);
        }
    }

    fn render_focused_help(&self, area: Rect, buf: &mut Buffer) {
        let field = self.focused_field();
        let status = self.field_status(field);
        let status_span = if status.1 {
            Span::from(status.0).green().dim()
        } else {
            Span::from(status.0).red().dim()
        };
        let help_width = area.width.saturating_sub(8) as usize;
        let help = crate::text_formatting::truncate_text(field.help(), help_width);
        Line::from(vec![
            "  ".into(),
            "hint".dim(),
            "  ".into(),
            status_span,
            "  ".into(),
            Span::from(help).dim(),
        ])
        .render_ref(area, buf);
    }

    fn field_status(&self, field: ProviderFieldKind) -> (&'static str, bool) {
        match field {
            ProviderFieldKind::Name => self.required_status(&self.name.value),
            ProviderFieldKind::BaseUrl => self.required_status(&self.base_url.value),
            ProviderFieldKind::WireApi => ("selected", true),
            ProviderFieldKind::ApiKey => self.required_status(&self.api_key.value),
            ProviderFieldKind::Models => self.required_status(&self.models.value),
            ProviderFieldKind::EnvKey => ("optional", true),
        }
    }

    fn required_status(&self, value: &str) -> (&'static str, bool) {
        if value.trim().is_empty() {
            ("required", false)
        } else {
            ("ready", true)
        }
    }
}

impl KeyboardHandler for CustomProvider {
    fn handle_key_event(&mut self, key_event: KeyEvent) {
        if !matches!(key_event.kind, KeyEventKind::Press | KeyEventKind::Repeat) {
            return;
        }
        match key_event.code {
            KeyCode::Esc => {
                self.result = Some(CustomProviderResult::Back);
            }
            KeyCode::Enter => {
                if let Err(msg) = self.submit() {
                    self.error = Some(msg);
                }
            }
            KeyCode::Up => {
                if !self.move_text_cursor_vertically(-1) {
                    self.move_focus(-1);
                }
            }
            KeyCode::Down => {
                if !self.move_text_cursor_vertically(1) {
                    self.move_focus(1);
                }
            }
            _ => match self.focused_field() {
                ProviderFieldKind::WireApi => self.handle_wire_api_key(&key_event),
                _ => self.handle_text_key(&key_event),
            },
        }
    }

    fn handle_paste(&mut self, pasted: String) {
        if pasted.is_empty() {
            return;
        }
        let field = self.focused_field();
        if field != ProviderFieldKind::WireApi {
            self.text_state_mut(field).insert_str(&pasted);
        }
    }
}

impl CustomProvider {
    fn handle_text_key(&mut self, key_event: &KeyEvent) {
        let state = self.text_state_mut(self.focused_field());
        match key_event.code {
            KeyCode::Char(c) => {
                state.insert_char(c);
            }
            KeyCode::Backspace => {
                state.delete_before();
            }
            KeyCode::Delete => {
                state.delete_after();
            }
            KeyCode::Left => {
                state.move_left();
            }
            KeyCode::Right => {
                state.move_right();
            }
            KeyCode::Home => {
                state.move_home();
            }
            KeyCode::End => {
                state.move_end();
            }
            _ => {}
        }
    }

    fn handle_wire_api_key(&mut self, key_event: &KeyEvent) {
        match key_event.code {
            KeyCode::Left => {
                self.wire_api = provider_ui::previous_wire_api(self.wire_api);
            }
            KeyCode::Right => {
                self.wire_api = provider_ui::next_wire_api(self.wire_api);
            }
            _ => {}
        }
    }
}

pub(crate) struct CustomProvider {
    name: TextState,
    base_url: TextState,
    wire_api: WireApi,
    api_key: TextState,
    models: TextState,
    focused: usize,
    error: Option<String>,
    result: Option<CustomProviderResult>,
    existing_names: Vec<String>,
    text_widths: [Cell<u16>; 4],
}

impl CustomProvider {
    pub(crate) fn new(existing_names: Vec<String>) -> Self {
        Self {
            name: TextState::new(String::new()),
            base_url: TextState::new(String::new()),
            wire_api: WireApi::Responses,
            api_key: TextState::new(String::new()),
            models: TextState::new(String::new()),
            focused: 0,
            error: None,
            result: None,
            existing_names,
            text_widths: std::array::from_fn(|_| Cell::new(1)),
        }
    }

    fn fields() -> &'static [ProviderFieldKind] {
        ProviderFormKind::OnboardingCustom.fields()
    }

    fn focused_field(&self) -> ProviderFieldKind {
        Self::fields()[self.focused]
    }

    fn is_submit_ready(&self) -> bool {
        !self.name.value.trim().is_empty()
            && !self.base_url.value.trim().is_empty()
            && !self.api_key.value.trim().is_empty()
            && !self.models.value.trim().is_empty()
    }

    fn move_focus(&mut self, delta: isize) {
        let len = Self::fields().len() as isize;
        self.focused = (self.focused as isize + delta).rem_euclid(len) as usize;
    }

    fn move_text_cursor_vertically(&mut self, line_delta: isize) -> bool {
        let field = self.focused_field();
        let Some(width) = self.field_text_width(field) else {
            return false;
        };
        let state = self.text_state_mut(field);
        let next = provider_ui::move_cursor_vertically_in_wrapped_text(
            &state.value,
            state.cursor,
            width.max(1),
            line_delta,
        );
        if next == state.cursor {
            false
        } else {
            state.cursor = next;
            true
        }
    }

    fn submit(&mut self) -> Result<(), String> {
        let name = self.name.value.trim().to_string();
        let base_url = self.base_url.value.trim().to_string();
        let api_key = self.api_key.value.trim().to_string();
        let models_str = self.models.value.trim().to_string();

        if name.is_empty() {
            return Err("Name is required".into());
        }
        if self.existing_names.iter().any(|n| n == &name) {
            return Err(format!("Provider '{name}' already exists"));
        }
        if base_url.is_empty() {
            return Err("Base URL is required".into());
        }
        provider_ui::validate_base_url(&base_url)?;
        if api_key.is_empty() {
            return Err("API Key is required".into());
        }
        let models = provider_ui::parse_models(&models_str)?;

        self.result = Some(CustomProviderResult::Saved {
            name,
            base_url,
            wire_api: self.wire_api,
            api_key,
            models,
        });
        Ok(())
    }

    pub(crate) fn take_result(&mut self) -> Option<CustomProviderResult> {
        self.result.take()
    }

    fn text_state(&self, field: ProviderFieldKind) -> &TextState {
        match field {
            ProviderFieldKind::Name => &self.name,
            ProviderFieldKind::BaseUrl => &self.base_url,
            ProviderFieldKind::ApiKey => &self.api_key,
            ProviderFieldKind::Models => &self.models,
            ProviderFieldKind::WireApi | ProviderFieldKind::EnvKey => unreachable!(),
        }
    }

    fn text_state_mut(&mut self, field: ProviderFieldKind) -> &mut TextState {
        match field {
            ProviderFieldKind::Name => &mut self.name,
            ProviderFieldKind::BaseUrl => &mut self.base_url,
            ProviderFieldKind::ApiKey => &mut self.api_key,
            ProviderFieldKind::Models => &mut self.models,
            ProviderFieldKind::WireApi | ProviderFieldKind::EnvKey => unreachable!(),
        }
    }

    fn field_text_width(&self, field: ProviderFieldKind) -> Option<u16> {
        Some(self.text_widths.get(field_text_width_index(field)?)?.get())
    }

    fn set_field_text_width(&self, field: ProviderFieldKind, width: u16) {
        if let Some(idx) = field_text_width_index(field)
            && let Some(cell) = self.text_widths.get(idx)
        {
            cell.set(width.max(1));
        }
    }
}

fn field_text_width_index(field: ProviderFieldKind) -> Option<usize> {
    match field {
        ProviderFieldKind::Name => Some(0),
        ProviderFieldKind::BaseUrl => Some(1),
        ProviderFieldKind::ApiKey => Some(2),
        ProviderFieldKind::Models => Some(3),
        ProviderFieldKind::WireApi | ProviderFieldKind::EnvKey => None,
    }
}

#[derive(Clone, Debug)]
struct TextState {
    value: String,
    cursor: usize,
}

impl TextState {
    fn new(value: String) -> Self {
        let len = value.chars().count();
        Self { value, cursor: len }
    }

    fn insert_char(&mut self, c: char) {
        let chars: Vec<char> = self.value.chars().collect();
        self.value = chars
            .iter()
            .take(self.cursor)
            .copied()
            .chain([c])
            .chain(chars.iter().skip(self.cursor).copied())
            .collect();
        self.cursor = self.cursor.saturating_add(1);
    }

    fn insert_str(&mut self, s: &str) {
        let chars: Vec<char> = self.value.chars().collect();
        self.value = chars
            .iter()
            .take(self.cursor)
            .copied()
            .chain(s.chars())
            .chain(chars.iter().skip(self.cursor).copied())
            .collect();
        self.cursor = self.cursor.saturating_add(s.chars().count());
    }

    fn delete_before(&mut self) {
        if self.cursor > 0 {
            let chars: Vec<char> = self.value.chars().collect();
            self.value = chars
                .iter()
                .take(self.cursor - 1)
                .copied()
                .chain(chars.iter().skip(self.cursor).copied())
                .collect();
            self.cursor = self.cursor.saturating_sub(1);
        }
    }

    fn delete_after(&mut self) {
        let len = self.value.chars().count();
        if self.cursor < len {
            let chars: Vec<char> = self.value.chars().collect();
            self.value = chars
                .iter()
                .take(self.cursor)
                .copied()
                .chain(chars.iter().skip(self.cursor + 1).copied())
                .collect();
        }
    }

    fn move_left(&mut self) {
        self.cursor = self.cursor.saturating_sub(1);
    }

    fn move_right(&mut self) {
        let len = self.value.chars().count();
        if self.cursor < len {
            self.cursor = self.cursor.saturating_add(1);
        }
    }

    fn move_home(&mut self) {
        self.cursor = 0;
    }

    fn move_end(&mut self) {
        self.cursor = self.value.chars().count();
    }
}
