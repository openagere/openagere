//! Shared provider TUI primitives.
//!
//! Keep provider-specific interaction details here so onboarding and `/provider`
//! render the same labels, cursor, masking, wrapping, and validation behavior.

use agere_config::config_toml::ModelConfig;
use agere_model_provider_info::WireApi;
use crossterm::event::KeyCode;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Stylize;
use ratatui::text::Line;
use ratatui::text::Span;
use ratatui::widgets::WidgetRef;
use std::ops::Range;
use textwrap::Options;
use unicode_width::UnicodeWidthChar;
use unicode_width::UnicodeWidthStr;

use crate::key_hint;
use crate::text_formatting::truncate_text;
use crate::wrapping::wrap_ranges_trim;

pub(crate) const PROVIDER_LABEL_WIDTH: usize = 12;
pub(crate) const WIRE_APIS: &[WireApi] = &[WireApi::Responses, WireApi::Chat, WireApi::Anthropic];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ProviderFieldKind {
    Name,
    BaseUrl,
    WireApi,
    EnvKey,
    ApiKey,
    Models,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ProviderFormKind {
    OnboardingCustom,
    WizardCustom,
    ExistingOrTemplate,
}

impl ProviderFormKind {
    pub(crate) fn fields(self) -> &'static [ProviderFieldKind] {
        match self {
            Self::OnboardingCustom => &[
                ProviderFieldKind::Name,
                ProviderFieldKind::BaseUrl,
                ProviderFieldKind::WireApi,
                ProviderFieldKind::ApiKey,
                ProviderFieldKind::Models,
            ],
            Self::WizardCustom => &[
                ProviderFieldKind::Name,
                ProviderFieldKind::BaseUrl,
                ProviderFieldKind::EnvKey,
                ProviderFieldKind::ApiKey,
                ProviderFieldKind::Models,
            ],
            Self::ExistingOrTemplate => &[ProviderFieldKind::ApiKey, ProviderFieldKind::Models],
        }
    }
}

impl ProviderFieldKind {
    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Name => "name",
            Self::BaseUrl => "base_url",
            Self::WireApi => "wire_api",
            Self::EnvKey => "env_key",
            Self::ApiKey => "API key",
            Self::Models => "models",
        }
    }

    pub(crate) fn placeholder(self) -> &'static str {
        match self {
            Self::Name => "my-provider",
            Self::BaseUrl => "https://api.example.com/v1",
            Self::WireApi => "",
            Self::EnvKey => "MY_PROVIDER_API_KEY",
            Self::ApiKey => "sk-...",
            Self::Models => "model-name[128k],reasoning-model[1m]",
        }
    }

    pub(crate) fn help(self) -> &'static str {
        match self {
            Self::Name => "Unique provider id used in config.toml",
            Self::BaseUrl => "e.g. https://api.openai.com/v1",
            Self::WireApi => "Use left/right to choose the wire protocol",
            Self::EnvKey => "Environment variable name (optional)",
            Self::ApiKey => "Paste your API key",
            Self::Models => "Format: name[context],name2[ctx]   k=1000 m=1M",
        }
    }
}

pub(crate) fn hint_spans(actions: &[(KeyCode, &'static str)]) -> Vec<Span<'static>> {
    let mut spans = vec!["  ".into()];
    for (idx, (key, label)) in actions.iter().enumerate() {
        if idx > 0 {
            spans.push("  ".dim());
        }
        spans.push(key_hint::plain(*key).into());
        spans.push(Span::from(format!(" {label}")).dim());
    }
    spans
}

pub(crate) fn hint_spans_with_alternates(
    actions: &[(&[KeyCode], &'static str)],
) -> Vec<Span<'static>> {
    let mut spans = vec!["  ".into()];
    for (idx, (keys, label)) in actions.iter().enumerate() {
        if idx > 0 {
            spans.push("  ".dim());
        }
        for (key_idx, key) in keys.iter().enumerate() {
            if key_idx > 0 {
                spans.push("/".dim());
            }
            spans.push(key_hint::plain(*key).into());
        }
        spans.push(Span::from(format!(" {label}")).dim());
    }
    spans
}

pub(crate) fn input_frame_text_area(area: Rect) -> Option<Rect> {
    if area.width <= 2 || area.height == 0 {
        return None;
    }
    Some(Rect {
        x: area.x.saturating_add(2),
        y: area.y,
        width: area.width.saturating_sub(2),
        height: area.height,
    })
}

pub(crate) fn input_frame_height(text: &str, width: u16) -> u16 {
    let text_width = width.saturating_sub(2).max(1);
    wrapped_line_count(text, text_width) as u16
}

pub(crate) fn render_input_frame(area: Rect, buf: &mut Buffer, text: &str, focused: bool) {
    if area.width == 0 || area.height == 0 {
        return;
    }
    let Some(text_area) = input_frame_text_area(area) else {
        return;
    };
    let ranges = wrap_ranges(text, text_area.width);
    for (line_idx, range) in ranges.iter().enumerate() {
        let y = text_area.y.saturating_add(line_idx as u16);
        if y >= area.y.saturating_add(area.height) {
            break;
        }
        let prompt = if line_idx == 0 {
            if focused { "› ".bold() } else { "› ".dim() }
        } else {
            "  ".into()
        };
        Line::from(vec![prompt, Span::from(text[range.clone()].to_string())]).render_ref(
            Rect {
                x: area.x,
                y,
                width: area.width,
                height: 1,
            },
            buf,
        );
    }
}

pub(crate) fn secret_input_card_height(text: &str, width: u16) -> u16 {
    let input_width = secret_input_text_frame_width(width);
    input_frame_height(text, input_width).saturating_add(3)
}

pub(crate) fn secret_input_card_text_area(area: Rect, text: &str) -> Option<Rect> {
    let frame_area = secret_input_text_frame_area(area, text)?;
    input_frame_text_area(frame_area)
}

pub(crate) fn render_secret_input_card(
    area: Rect,
    buf: &mut Buffer,
    title: &str,
    text: &str,
    focused: bool,
    save_enabled: bool,
) {
    if area.width < 8 || area.height == 0 {
        return;
    }

    let card_width = area.width as usize;
    let border_width = card_width.saturating_sub(2).max(2);
    render_card_top(area, buf, title, border_width);

    if area.height <= 1 {
        return;
    }

    let input_height = input_frame_height(text, secret_input_text_frame_width(area.width));
    for line_idx in 0..input_height {
        render_card_line(area, buf, line_idx.saturating_add(1), &[]);
    }
    if let Some(frame_area) = secret_input_text_frame_area(area, text) {
        render_input_frame(frame_area, buf, text, focused);
    }

    let hint_y = input_height.saturating_add(1);
    if hint_y < area.height {
        let mut spans = if save_enabled {
            Vec::new()
        } else {
            vec!["Enter an API key to save".dim()]
        };
        let hints = if save_enabled {
            hint_spans(&[(KeyCode::Enter, "save"), (KeyCode::Esc, "cancel")])
        } else {
            hint_spans(&[(KeyCode::Esc, "cancel")])
        };
        spans.push("  ".dim());
        spans.extend(hints.into_iter().skip(1));
        render_card_line(area, buf, hint_y, &spans);
    }

    let bottom_y = input_height.saturating_add(2);
    if bottom_y < area.height {
        Line::from(format!(
            "  ╰{}╯",
            "─".repeat(border_width.saturating_sub(2))
        ))
        .fg(crate::brand_card::BORDER_COLOR)
        .render_ref(
            Rect {
                y: area.y.saturating_add(bottom_y),
                ..area
            },
            buf,
        );
    }
}

fn secret_input_text_frame_width(card_width: u16) -> u16 {
    card_width.saturating_sub(6).max(1)
}

fn secret_input_text_frame_area(area: Rect, text: &str) -> Option<Rect> {
    if area.width < 8 || area.height <= 1 {
        return None;
    }
    let width = secret_input_text_frame_width(area.width);
    Some(Rect {
        x: area.x.saturating_add(4),
        y: area.y.saturating_add(1),
        width,
        height: input_frame_height(text, width).min(area.height.saturating_sub(1)),
    })
}

fn render_card_top(area: Rect, buf: &mut Buffer, title: &str, border_width: usize) {
    let title = truncate_text(title, border_width.saturating_sub(4));
    let used_width = UnicodeWidthStr::width("╭─ ") + UnicodeWidthStr::width(title.as_str()) + 1;
    let rule_width = border_width.saturating_sub(used_width).saturating_sub(1);
    Line::from(vec![
        "  ╭─ ".fg(crate::brand_card::BORDER_COLOR),
        Span::from(title).cyan().bold(),
        Span::from(format!(" {}╮", "─".repeat(rule_width))).fg(crate::brand_card::BORDER_COLOR),
    ])
    .render_ref(area, buf);
}

fn render_card_line(area: Rect, buf: &mut Buffer, line_idx: u16, content: &[Span<'static>]) {
    if line_idx >= area.height {
        return;
    }

    let inner_width = area.width.saturating_sub(6) as usize;
    let used_width: usize = content
        .iter()
        .map(|span| UnicodeWidthStr::width(span.content.as_ref()))
        .sum();
    let mut spans = vec!["  │ ".fg(crate::brand_card::BORDER_COLOR)];
    spans.extend_from_slice(content);
    spans.push(Span::from(" ".repeat(inner_width.saturating_sub(used_width))).dim());
    spans.push(" │".fg(crate::brand_card::BORDER_COLOR));

    Line::from(spans).render_ref(
        Rect {
            y: area.y.saturating_add(line_idx),
            ..area
        },
        buf,
    );
}

pub(crate) fn parse_context_window(input: &str) -> Option<i64> {
    let input = input.trim().to_lowercase();
    if input.is_empty() {
        return None;
    }
    let number_end = input
        .find(|ch: char| !ch.is_ascii_digit())
        .unwrap_or(input.len());
    let number: i64 = input[..number_end].parse().ok()?;
    let multiplier = match &input[number_end..] {
        "" => 1,
        "k" => 1_000,
        "m" => 1_000_000,
        "g" => 1_000_000_000,
        _ => return None,
    };
    let value = number.checked_mul(multiplier)?;
    if value <= 0 { None } else { Some(value) }
}

pub(crate) fn format_context_window(context_window: i64) -> String {
    if context_window % 1_000_000 == 0 {
        format!("{}m", context_window / 1_000_000)
    } else if context_window % 1_000 == 0 {
        format!("{}k", context_window / 1_000)
    } else {
        context_window.to_string()
    }
}

pub(crate) fn parse_models(input: &str) -> Result<Vec<ModelConfig>, String> {
    let mut models = Vec::new();
    for part in input.split(',') {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        if let Some(bracket) = part.find('[') {
            let name = part[..bracket].trim();
            if name.is_empty() {
                return Err("model name is required before [context]".into());
            }
            if !part.ends_with(']') {
                return Err(format!("model '{name}' is missing closing ]"));
            }
            let context = part[bracket + 1..part.len() - 1].trim();
            let Some(context_window) = parse_context_window(context) else {
                return Err(format!(
                    "invalid context window for model '{name}': use digits with optional k, m, or g"
                ));
            };
            models.push(ModelConfig {
                name: name.to_string(),
                context_window: Some(context_window),
            });
        } else {
            if part.contains(']') {
                return Err(format!("model '{part}' has ] without ["));
            }
            models.push(ModelConfig {
                name: part.to_string(),
                context_window: None,
            });
        }
    }
    if models.is_empty() {
        return Err("at least one model is required".into());
    }
    Ok(models)
}

pub(crate) fn models_to_string(models: &[ModelConfig]) -> String {
    models
        .iter()
        .map(|model| {
            if let Some(context_window) = model.context_window {
                format!("{}[{}]", model.name, format_context_window(context_window))
            } else {
                model.name.clone()
            }
        })
        .collect::<Vec<_>>()
        .join(",")
}

pub(crate) fn mask_key(key: &str) -> String {
    let len = key.chars().count();
    if len <= 8 {
        return "•".repeat(len);
    }
    let head: String = key.chars().take(4).collect();
    let tail: String = key.chars().skip(len - 4).collect();
    format!("{head}…{tail}")
}

pub(crate) fn wrapped_line_count(text: &str, width: u16) -> usize {
    textwrap::wrap(
        text,
        Options::new(width.max(1) as usize)
            .wrap_algorithm(textwrap::WrapAlgorithm::FirstFit)
            .break_words(true),
    )
    .len()
    .max(1)
}

pub(crate) fn wrap_ranges(text: &str, width: u16) -> Vec<Range<usize>> {
    let ranges = wrap_ranges_trim(
        text,
        Options::new(width.max(1) as usize)
            .wrap_algorithm(textwrap::WrapAlgorithm::FirstFit)
            .break_words(true),
    );
    if ranges.is_empty() {
        std::iter::once(0..0).collect()
    } else {
        ranges
    }
}

pub(crate) fn cursor_pos_for_wrapped_text(
    text: &str,
    cursor_pos: usize,
    area: Rect,
) -> Option<(u16, u16)> {
    if area.width == 0 || area.height == 0 {
        return None;
    }
    let ranges = wrap_ranges(text, area.width);
    let cursor_byte = text
        .char_indices()
        .nth(cursor_pos)
        .map(|(idx, _)| idx)
        .unwrap_or(text.len());
    for (line_idx, range) in ranges.iter().enumerate() {
        if range.start <= cursor_byte
            && (cursor_byte < range.end
                || (line_idx == ranges.len() - 1 && cursor_byte == range.end))
        {
            let column = text[range.start..cursor_byte].width() as u16;
            return Some((
                area.x
                    .saturating_add(column.min(area.width.saturating_sub(1))),
                area.y.saturating_add(line_idx as u16),
            ));
        }
    }
    None
}

pub(crate) fn move_cursor_vertically_in_wrapped_text(
    text: &str,
    cursor_pos: usize,
    width: u16,
    line_delta: isize,
) -> usize {
    if width == 0 || line_delta == 0 {
        return cursor_pos;
    }
    let ranges = wrap_ranges(text, width);
    let cursor_byte = byte_pos_for_char_pos(text, cursor_pos);
    let Some(current_line_idx) = ranges.iter().enumerate().find_map(|(idx, range)| {
        (range.start <= cursor_byte
            && (cursor_byte < range.end || (idx == ranges.len() - 1 && cursor_byte == range.end)))
            .then_some(idx)
    }) else {
        return cursor_pos;
    };
    let target_line_idx = current_line_idx.saturating_add_signed(line_delta);
    let Some(target_range) = ranges.get(target_line_idx) else {
        return cursor_pos;
    };
    let current_range = &ranges[current_line_idx];
    let target_column = text[current_range.start..cursor_byte].width();
    char_pos_for_visual_column(text, target_range, target_column)
}

fn byte_pos_for_char_pos(text: &str, cursor_pos: usize) -> usize {
    text.char_indices()
        .nth(cursor_pos)
        .map(|(idx, _)| idx)
        .unwrap_or(text.len())
}

fn char_pos_for_byte_pos(text: &str, byte_pos: usize) -> usize {
    text[..byte_pos].chars().count()
}

fn char_pos_for_visual_column(text: &str, range: &Range<usize>, target_column: usize) -> usize {
    let line = &text[range.clone()];
    let mut column = 0usize;
    for (rel_idx, ch) in line.char_indices() {
        let ch_start = range.start + rel_idx;
        let ch_end = ch_start + ch.len_utf8();
        let next_column = column.saturating_add(ch.width().unwrap_or(0));
        if target_column <= column {
            return char_pos_for_byte_pos(text, ch_start);
        }
        if target_column < next_column {
            let before_delta = target_column.saturating_sub(column);
            let after_delta = next_column.saturating_sub(target_column);
            let byte_pos = if before_delta <= after_delta {
                ch_start
            } else {
                ch_end
            };
            return char_pos_for_byte_pos(text, byte_pos);
        }
        column = next_column;
    }
    char_pos_for_byte_pos(text, range.end)
}

pub(crate) fn wire_api_name(api: WireApi) -> &'static str {
    match api {
        WireApi::Responses => "Responses",
        WireApi::Chat => "Chat",
        WireApi::Anthropic => "Anthropic",
    }
}

pub(crate) fn next_wire_api(current: WireApi) -> WireApi {
    let idx = WIRE_APIS
        .iter()
        .position(|api| *api == current)
        .unwrap_or(0);
    WIRE_APIS[(idx + 1) % WIRE_APIS.len()]
}

pub(crate) fn previous_wire_api(current: WireApi) -> WireApi {
    let idx = WIRE_APIS
        .iter()
        .position(|api| *api == current)
        .unwrap_or(0);
    WIRE_APIS[(idx + WIRE_APIS.len() - 1) % WIRE_APIS.len()]
}

pub(crate) fn validate_base_url(input: &str) -> Result<(), String> {
    if input.is_empty() {
        return Err("base_url is required".into());
    }
    if !input.starts_with("http://") && !input.starts_with("https://") {
        return Err("base_url must start with http:// or https://".into());
    }
    Ok(())
}

pub(crate) fn validate_api_key(input: &str) -> Result<(), String> {
    if input.is_empty() {
        return Err("API key is required".into());
    }
    Ok(())
}

pub(crate) fn validate_models(input: &str) -> Result<(), String> {
    if input.is_empty() {
        return Err("at least one model is required".into());
    }
    parse_models(input)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use ratatui::style::Modifier;

    #[test]
    fn onboarding_custom_fields_include_base_url_without_env_key() {
        assert_eq!(
            ProviderFormKind::OnboardingCustom.fields(),
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
    fn model_roundtrip_uses_provider_format() {
        let models = parse_models("gpt-4o[200k],claude,deepseek[2M]").expect("valid models");
        assert_eq!(
            models,
            vec![
                ModelConfig {
                    name: "gpt-4o".to_string(),
                    context_window: Some(200_000),
                },
                ModelConfig {
                    name: "claude".to_string(),
                    context_window: None,
                },
                ModelConfig {
                    name: "deepseek".to_string(),
                    context_window: Some(2_000_000),
                },
            ]
        );
        assert_eq!(
            models_to_string(&models),
            "gpt-4o[200k],claude,deepseek[2m]"
        );
    }

    #[test]
    fn model_context_validation_rejects_invalid_brackets() {
        assert_eq!(
            parse_models("gpt-4o[abc]").unwrap_err(),
            "invalid context window for model 'gpt-4o': use digits with optional k, m, or g"
        );
        assert_eq!(
            parse_models("gpt-4o[128k").unwrap_err(),
            "model 'gpt-4o' is missing closing ]"
        );
        assert_eq!(
            parse_models("gpt-4o]").unwrap_err(),
            "model 'gpt-4o]' has ] without ["
        );
    }

    #[test]
    fn vertical_cursor_movement_uses_wrapped_visual_lines() {
        let text = "abcdefghi";
        assert_eq!(move_cursor_vertically_in_wrapped_text(text, 7, 3, -1), 4);
        assert_eq!(move_cursor_vertically_in_wrapped_text(text, 1, 3, 1), 4);
    }

    #[test]
    fn vertical_cursor_movement_clamps_to_shorter_target_line() {
        let text = "abcdefg";
        assert_eq!(move_cursor_vertically_in_wrapped_text(text, 5, 3, 1), 7);
    }

    #[test]
    fn secret_input_card_uses_full_intensity_border_glyphs() {
        let area = Rect::new(0, 0, 32, 4);
        let mut buf = Buffer::empty(area);

        render_secret_input_card(area, &mut buf, "API key", "sk-test", true, true);

        let top_left = &buf[(2, 0)];
        let top_right = &buf[(31, 0)];
        let left = &buf[(2, 1)];
        let right = &buf[(31, 1)];
        let bottom_left = &buf[(2, 3)];
        let bottom_right = &buf[(31, 3)];

        assert_eq!(top_left.symbol(), "╭");
        assert_eq!(top_right.symbol(), "╮");
        assert_eq!(left.symbol(), "│");
        assert_eq!(right.symbol(), "│");
        assert_eq!(bottom_left.symbol(), "╰");
        assert_eq!(bottom_right.symbol(), "╯");
        assert!(!top_left.modifier.contains(Modifier::DIM));
        assert!(!top_right.modifier.contains(Modifier::DIM));
        assert!(!left.modifier.contains(Modifier::DIM));
        assert!(!right.modifier.contains(Modifier::DIM));
        assert!(!bottom_left.modifier.contains(Modifier::DIM));
        assert!(!bottom_right.modifier.contains(Modifier::DIM));
        assert_eq!(top_left.fg, crate::brand_card::BORDER_COLOR);
        assert_eq!(top_right.fg, crate::brand_card::BORDER_COLOR);
        assert_eq!(left.fg, crate::brand_card::BORDER_COLOR);
        assert_eq!(right.fg, crate::brand_card::BORDER_COLOR);
        assert_eq!(bottom_left.fg, crate::brand_card::BORDER_COLOR);
        assert_eq!(bottom_right.fg, crate::brand_card::BORDER_COLOR);
    }
}
