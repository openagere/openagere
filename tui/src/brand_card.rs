use ratatui::style::Stylize;
use ratatui::text::Line;
use ratatui::text::Span;
use unicode_width::UnicodeWidthStr;

use crate::text_formatting::truncate_text;

pub(crate) const BORDER_COLOR: ratatui::style::Color = ratatui::style::Color::DarkGray;

const BRAND_MARK: &str = "✦";
const BRAND_NAME: &str = "OpenAgere";
pub(crate) const BRAND_LINK_LINE: &str = "learn more at https://openagere.com";
const BRAND_LINK_PREFIX: &str = "learn more at ";
const BRAND_LINK_URL: &str = "https://openagere.com";

pub(crate) fn titled_top_border(border_inner_width: usize) -> Line<'static> {
    let top_left = "╭─ ";
    let top_right = " ╮";
    let title = format!("{BRAND_MARK} {BRAND_NAME}");
    let line_width = border_inner_width.saturating_add(2);
    let title_width = line_width
        .saturating_sub(UnicodeWidthStr::width(top_left))
        .saturating_sub(UnicodeWidthStr::width(top_right))
        .max(1);
    let title = truncate_text(&title, title_width);
    let used_width = UnicodeWidthStr::width(top_left)
        + UnicodeWidthStr::width(title.as_str())
        + UnicodeWidthStr::width(top_right);
    let rule_width = line_width.saturating_sub(used_width);

    Line::from(vec![
        Span::from(top_left).fg(BORDER_COLOR),
        Span::from(title).green().bold(),
        Span::from(format!(" {}", "─".repeat(rule_width))).fg(BORDER_COLOR),
        Span::from("╮").fg(BORDER_COLOR),
    ])
}

pub(crate) fn brand_body_lines(width: usize) -> Vec<Line<'static>> {
    let separator_width = width.max(1);
    vec![
        brand_link_line(width),
        Span::from("─".repeat(separator_width))
            .fg(BORDER_COLOR)
            .into(),
    ]
}

fn brand_link_line(width: usize) -> Line<'static> {
    Line::from(brand_link_spans(width))
}

fn brand_link_spans(width: usize) -> Vec<Span<'static>> {
    let prefix_width = UnicodeWidthStr::width(BRAND_LINK_PREFIX);
    if width <= prefix_width {
        return vec![Span::from(truncate_text(BRAND_LINK_LINE, width)).dim()];
    }

    vec![
        Span::from(BRAND_LINK_PREFIX).dim(),
        Span::from(truncate_text(BRAND_LINK_URL, width - prefix_width))
            .cyan()
            .underlined(),
    ]
}
