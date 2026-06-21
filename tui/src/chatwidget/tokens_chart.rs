//! Renders provider-based usage summaries and activity charts for `/usage`.
//!
//! This module owns the chart data bucketing and ratatui line construction. The
//! async card lifecycle stays in the parent [`super::tokens`] module so chart
//! rendering remains a pure transformation from loaded [`UsageData`].

use std::collections::BTreeMap;

use chrono::Datelike;
use chrono::Duration;
use chrono::NaiveDate;
use ratatui::style::Color;
use ratatui::style::Style;
use ratatui::style::Stylize;
use ratatui::text::Line;
use ratatui::text::Span;

use super::tokens::UsageData;
use crate::color::blend;
use crate::color::is_light;
use crate::render::highlight::foreground_style_for_scopes;
use crate::status::format_tokens_compact;
use crate::style::accent_style;
use crate::terminal_palette::StdoutColorLevel;
use crate::terminal_palette::best_color_for_level;
use crate::terminal_palette::default_bg;
use crate::terminal_palette::default_fg;
use crate::terminal_palette::stdout_color_level;

const WEEK_COUNT: usize = 52;
const DAY_COUNT: usize = 7;
const CELL_COUNT: usize = WEEK_COUNT * DAY_COUNT;
const CHART_LEFT_WIDTH: usize = 4;
const SUMMARY_INDENT: &str = " ";
const SUMMARY_INDENT_WIDTH: u16 = 1;

const EMPTY_CELL_GLYPH: &str = "□";
const ACTIVE_CELL_GLYPH: &str = "■";
const BAR_CELL_GLYPH: &str = "█";

/// Selects the aggregation represented by the token activity chart.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum TokenActivityView {
    Daily,
    Weekly,
    Cumulative,
}

impl TokenActivityView {
    /// Parses the optional `/usage` argument into a supported chart view.
    ///
    /// An empty argument selects the daily view so `/usage` and `/usage daily`
    /// behave identically. Returning `None` lets the slash-command dispatcher
    /// report unsupported arguments instead of silently choosing a view.
    pub(crate) fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "" | "day" | "daily" => Some(Self::Daily),
            "week" | "weekly" => Some(Self::Weekly),
            "cumulative" => Some(Self::Cumulative),
            _ => None,
        }
    }

    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Daily => "Daily",
            Self::Weekly => "Weekly",
            Self::Cumulative => "Cumulative",
        }
    }
}

/// Terminal-adaptive styles and glyph choices for the activity chart.
struct ChartPalette {
    styles: [Style; 5],
    bar_style: Style,
    uses_color: bool,
}

impl ChartPalette {
    fn current() -> Self {
        Self::from_parts(
            default_fg(),
            default_bg(),
            stdout_color_level(),
            theme_activity_style(),
        )
    }

    fn from_parts(
        default_fg: Option<(u8, u8, u8)>,
        default_bg: Option<(u8, u8, u8)>,
        color_level: StdoutColorLevel,
        active_style: Style,
    ) -> Self {
        let fallback_palette = || Self::fallback(active_style);
        let (Some(fg), Some(bg), Some(anchor)) =
            (default_fg, default_bg, activity_anchor_rgb(active_style))
        else {
            return fallback_palette();
        };
        if matches!(
            color_level,
            StdoutColorLevel::Ansi16 | StdoutColorLevel::Unknown
        ) {
            return fallback_palette();
        }
        let empty_alpha = if is_light(bg) { 0.18 } else { 0.14 };
        let alphas = [empty_alpha, 0.22, 0.42, 0.68, 1.00];
        let styles = std::array::from_fn(|index| {
            let color = if index == 0 {
                blend(fg, bg, alphas[index])
            } else {
                blend(anchor, bg, alphas[index])
            };
            Style::default().fg(best_color_for_level(color, color_level))
        });
        let bar_style =
            Style::default().fg(best_color_for_level(blend(anchor, bg, 0.78), color_level));
        Self {
            styles,
            bar_style,
            uses_color: true,
        }
    }

    fn fallback(active_style: Style) -> Self {
        let empty_style = Style::default().dim();
        Self {
            styles: [
                empty_style,
                active_style,
                active_style,
                active_style,
                active_style,
            ],
            bar_style: active_style,
            uses_color: false,
        }
    }

    fn for_level(&self, level: usize) -> Style {
        self.styles[level.min(4)]
    }

    fn for_bar_level(&self, level: usize) -> Style {
        if level == 0 {
            self.for_level(0)
        } else {
            self.bar_style
        }
    }

    fn glyph(&self, view: TokenActivityView, level: usize) -> &'static str {
        if view != TokenActivityView::Daily {
            return if level == 0 { " " } else { BAR_CELL_GLYPH };
        }
        if self.uses_color || level > 0 {
            ACTIVE_CELL_GLYPH
        } else {
            EMPTY_CELL_GLYPH
        }
    }
}

fn theme_activity_style() -> Style {
    foreground_style_for_scopes(&["entity.name.type", "support.type", "variable"])
        .unwrap_or_else(accent_style)
}

fn activity_anchor_rgb(style: Style) -> Option<(u8, u8, u8)> {
    match style.fg? {
        Color::Rgb(r, g, b) => Some((r, g, b)),
        _ => None,
    }
}

fn numeric_style() -> Style {
    foreground_style_for_scopes(&["constant.numeric", "constant"])
        .unwrap_or_else(|| Style::default().green())
}

fn label_style() -> Style {
    foreground_style_for_scopes(&["comment"]).unwrap_or_else(|| Style::default().dim())
}

pub(crate) fn loaded_lines(
    view: TokenActivityView,
    data: &UsageData,
    today: NaiveDate,
    width: u16,
) -> Vec<Line<'static>> {
    let mut lines = vec![
        vec![
            Span::from(" Token activity").bold(),
            Span::styled("   last 12 months", label_style()),
        ]
        .into(),
    ];
    lines.extend(summary_lines(data, graph_width(width)));
    lines.push(Line::default());
    let daily_totals = data.daily_totals();
    if daily_totals.is_empty() {
        lines.push("   Token activity history unavailable".dim().into());
        return lines;
    }
    lines.extend(chart_lines(view, &daily_totals, today, width));
    lines
}

fn chart_lines(
    view: TokenActivityView,
    daily_totals: &[(String, i64)],
    today: NaiveDate,
    width: u16,
) -> Vec<Line<'static>> {
    let mut lines = Vec::new();
    let values = daily_values(daily_totals, today);
    let shown_columns = shown_columns(width);
    if shown_columns == 0 {
        lines.push(Span::styled("   Widen terminal to show activity graph", label_style()).into());
        return lines;
    }
    let palette = ChartPalette::current();
    let levels = levels_for_view(&values, view);
    let first_column = WEEK_COUNT - shown_columns;
    lines.push(month_labels(today, first_column, shown_columns));
    for row in 0..DAY_COUNT {
        let mut spans = vec![weekday_label(view, row)];
        for column in first_column..WEEK_COUNT {
            if column > first_column {
                spans.push(" ".into());
            }
            let index = column * DAY_COUNT + row;
            if view == TokenActivityView::Daily
                && cell_date(today, index).is_some_and(|date| date > today)
            {
                spans.push(" ".into());
            } else {
                let style = if view == TokenActivityView::Daily {
                    palette.for_level(levels[index])
                } else {
                    palette.for_bar_level(levels[index])
                };
                spans.push(Span::styled(palette.glyph(view, levels[index]), style));
            }
        }
        lines.push(spans.into());
    }
    lines.push(Line::default());
    match view {
        TokenActivityView::Daily => lines.push(legend_line(&palette)),
        TokenActivityView::Weekly | TokenActivityView::Cumulative => {
            lines.push(bar_caption(view, &values))
        }
    }
    lines.push(view_footer(view));
    lines
}

fn shown_columns(width: u16) -> usize {
    (usize::from(width)
        .saturating_sub(CHART_LEFT_WIDTH)
        .saturating_add(1)
        / 2)
    .min(WEEK_COUNT)
}

fn graph_width(width: u16) -> u16 {
    if width == u16::MAX {
        return width;
    }
    (CHART_LEFT_WIDTH + shown_columns(width) * 2 - 1) as u16
}

fn summary_lines(data: &UsageData, width: u16) -> Vec<Line<'static>> {
    let total = &data.response.total;
    let streak = compute_streaks(&data.daily_totals());
    let mut fields = vec![
        ("Total", format_tokens_compact(total.total_tokens)),
        ("Peak", format_tokens_compact(total.peak_daily_tokens)),
        ("Streak", format_streak(streak.current, streak.longest)),
        (
            "Longest task",
            format_optional_duration(data.longest_running_turn_sec()),
        ),
    ];
    for provider in &data.response.providers {
        fields.push((
            &provider.provider_id,
            format_tokens_compact(provider.total_tokens),
        ));
    }
    pack_fields(&fields, width)
        .into_iter()
        .map(|group| align_summary_line(summary_line(&fields, &group), width))
        .collect()
}

/// Compute current and longest active-day streaks from date/token pairs.
fn compute_streaks(daily_totals: &[(String, i64)]) -> Streaks {
    let mut current: u64 = 0;
    let mut longest: u64 = 0;
    let mut running: u64 = 0;
    let mut prev_date: Option<NaiveDate> = None;
    let today = chrono::Utc::now().date_naive();
    for (date_str, tokens) in daily_totals {
        let Ok(date) = NaiveDate::parse_from_str(date_str, "%Y-%m-%d") else {
            continue;
        };
        if *tokens <= 0 {
            continue;
        }
        match prev_date {
            Some(prev) if date == prev + Duration::days(1) => {
                running += 1;
            }
            _ => {
                running = 1;
            }
        }
        prev_date = Some(date);
        if running > longest {
            longest = running;
        }
    }
    // Current streak: count backwards from today
    if let Some(prev) = prev_date {
        if prev == today || prev == today - Duration::days(1) {
            current = running;
        }
    }
    Streaks { current, longest }
}

struct Streaks {
    current: u64,
    longest: u64,
}

/// Combine the current and longest streak into one field: a bare `54d` when
/// they match, otherwise `12d (best 54d)`.
fn format_streak(current: u64, longest: u64) -> String {
    match (current, longest) {
        (0, 0) => "-".to_string(),
        (c, l) if c == l => format!("{c}d"),
        (c, l) => format!("{c}d (best {l}d)"),
    }
}

/// Format an optional turn duration in seconds as a human-readable string.
///
/// Returns `"-"` when no duration is available, otherwise abbreviates to
/// hours/minutes/seconds (for example `"3h 52m"`, `"12m"`, `"45s"`).
fn format_optional_duration(value: Option<i64>) -> String {
    let Some(seconds) = value else {
        return "-".to_string();
    };
    let seconds = seconds.max(0);
    let hours = seconds / 3600;
    let minutes = (seconds % 3600) / 60;
    let secs = seconds % 60;
    match (hours, minutes, secs) {
        (0, 0, 0) => "-".to_string(),
        (0, 0, s) => format!("{s}s"),
        (0, m, 0) => format!("{m}m"),
        (0, m, s) => format!("{m}m {s}s"),
        (h, 0, 0) => format!("{h}h"),
        (h, m, 0) => format!("{h}h {m}m"),
        (h, m, s) => format!("{h}h {m}m {s}s"),
    }
}

/// Greedily pack summary fields into as few lines as fit `width`,
/// keeping field order. `u16::MAX` (raw/copy mode) always yields one line.
fn pack_fields(fields: &[(&str, String)], width: u16) -> Vec<Vec<usize>> {
    if width == u16::MAX {
        return vec![(0..fields.len()).collect()];
    }
    let max = usize::from(width.saturating_sub(SUMMARY_INDENT_WIDTH));
    let mut groups: Vec<Vec<usize>> = Vec::new();
    let mut current: Vec<usize> = Vec::new();
    for index in 0..fields.len() {
        let mut candidate = current.clone();
        candidate.push(index);
        if !current.is_empty() && summary_line(fields, &candidate).width() > max {
            groups.push(std::mem::take(&mut current));
            current.push(index);
        } else {
            current = candidate;
        }
    }
    if !current.is_empty() {
        groups.push(current);
    }
    groups
}

fn summary_line(fields: &[(&str, String)], indexes: &[usize]) -> Line<'static> {
    let mut spans = Vec::new();
    for (index, field_index) in indexes.iter().enumerate() {
        if index > 0 {
            spans.push(Span::styled(" · ", label_style()));
        }
        let (label, value) = &fields[*field_index];
        spans.push(Span::styled(format!("{label} "), label_style()));
        spans.push(Span::styled(value.clone(), numeric_style()));
    }
    spans.into()
}

fn align_summary_line(mut line: Line<'static>, width: u16) -> Line<'static> {
    if width == u16::MAX {
        return line;
    }
    line.spans.insert(0, SUMMARY_INDENT.into());
    line
}

fn weekday_label(view: TokenActivityView, row: usize) -> Span<'static> {
    if view != TokenActivityView::Daily {
        return match row {
            0 => Span::styled("max ", label_style()),
            6 => Span::styled("  0 ", label_style()),
            _ => Span::styled("    ", label_style()),
        }
        .into();
    }
    match row {
        0 => Span::styled(" Su ", label_style()),
        1 => Span::styled(" Mo ", label_style()),
        2 => Span::styled(" Tu ", label_style()),
        3 => Span::styled(" We ", label_style()),
        4 => Span::styled(" Th ", label_style()),
        5 => Span::styled(" Fr ", label_style()),
        6 => Span::styled(" Sa ", label_style()),
        _ => Span::styled("    ", label_style()),
    }
    .into()
}

fn legend_line(palette: &ChartPalette) -> Line<'static> {
    let mut spans = vec![Span::styled("   Less ", label_style())];
    for level in 0..=4 {
        if level > 0 {
            spans.push(" ".into());
        }
        spans.push(Span::styled(
            palette.glyph(TokenActivityView::Daily, level),
            palette.for_level(level),
        ));
    }
    spans.push(Span::styled(" More", label_style()));
    spans.into()
}

fn bar_caption(view: TokenActivityView, values: &[i64]) -> Line<'static> {
    let weeks = weekly_totals(values);
    let (lead, peak) = match view {
        TokenActivityView::Weekly => (
            "Each column = 1 week · tallest ",
            weeks.iter().copied().max().unwrap_or(0),
        ),
        TokenActivityView::Cumulative => ("Running total · top ", weeks.iter().sum::<i64>()),
        TokenActivityView::Daily => ("", 0),
    };
    if peak <= 0 {
        return Span::styled("   No token activity in the last 12 months", label_style()).into();
    }
    vec![
        Span::styled(format!("   {lead}"), label_style()),
        Span::styled(format_tokens_compact(peak), numeric_style()),
    ]
    .into()
}

/// Dim footer that surfaces the other `/usage` views and emphasizes the
/// active one, so the weekly/cumulative modes are discoverable from the card.
fn view_footer(active: TokenActivityView) -> Line<'static> {
    let mut spans = vec![Span::styled("   ", label_style())];
    for (index, (view, name)) in [
        (TokenActivityView::Daily, "daily"),
        (TokenActivityView::Weekly, "weekly"),
        (TokenActivityView::Cumulative, "cumulative"),
    ]
    .into_iter()
    .enumerate()
    {
        if index > 0 {
            spans.push(Span::styled(" · ", label_style()));
        }
        let style = if view == active {
            numeric_style().bold()
        } else {
            label_style()
        };
        spans.push(Span::styled(name, style));
    }
    spans.into()
}

fn month_labels(today: NaiveDate, first_column: usize, shown_columns: usize) -> Line<'static> {
    let mut cells = vec![' '; shown_columns * 2 - 1];
    let start = chart_start(today);
    let mut last_end = 0;
    for column in first_column..WEEK_COUNT {
        let date = start + Duration::days((column * DAY_COUNT) as i64);
        if date.day() > 7 {
            continue;
        }
        let label = date.format("%b").to_string();
        let offset = (column - first_column) * 2;
        if offset < last_end || offset + label.len() > cells.len() {
            continue;
        }
        for (index, ch) in label.chars().enumerate() {
            cells[offset + index] = ch;
        }
        last_end = offset + label.len() + 1;
    }
    vec![
        "    ".into(),
        Span::styled(cells.into_iter().collect::<String>(), label_style()),
    ]
    .into()
}

/// Normalizes daily totals into the fixed 52-week display window.
///
/// The returned vector is ordered by chart cell, starting with the oldest
/// Sunday. Invalid, out-of-window, and future dates are ignored. Duplicate
/// dates are accumulated and negative token values do not reduce activity.
fn daily_values(daily_totals: &[(String, i64)], today: NaiveDate) -> Vec<i64> {
    let start = chart_start(today);
    let end = start + Duration::days(CELL_COUNT as i64);
    let mut by_date = BTreeMap::new();
    for (date_str, tokens) in daily_totals {
        let Ok(date) = NaiveDate::parse_from_str(date_str, "%Y-%m-%d") else {
            continue;
        };
        if date < start || date >= end || date > today {
            continue;
        }
        *by_date.entry(date).or_insert(0) += (*tokens).max(0);
    }
    (0..CELL_COUNT)
        .map(|offset| {
            by_date
                .get(&(start + Duration::days(offset as i64)))
                .copied()
                .unwrap_or(0)
        })
        .collect()
}

fn levels_for_view(values: &[i64], view: TokenActivityView) -> Vec<usize> {
    match view {
        TokenActivityView::Daily => graded_levels(values),
        TokenActivityView::Weekly => bar_levels(&weekly_totals(values)),
        TokenActivityView::Cumulative => {
            let cumulative = weekly_totals(values)
                .into_iter()
                .scan(0, |sum, value| {
                    *sum += value;
                    Some(*sum)
                })
                .collect::<Vec<_>>();
            bar_levels(&cumulative)
        }
    }
}

fn graded_levels(values: &[i64]) -> Vec<usize> {
    let max = values.iter().copied().max().unwrap_or(0);
    values
        .iter()
        .map(|value| match (*value, max) {
            (0, _) | (_, 0) => 0,
            (value, max) if value * 4 > max * 3 => 4,
            (value, max) if value * 2 > max => 3,
            (value, max) if value * 4 > max => 2,
            _ => 1,
        })
        .collect()
}

fn weekly_totals(values: &[i64]) -> Vec<i64> {
    values
        .chunks(DAY_COUNT)
        .map(|week| week.iter().sum())
        .collect()
}

fn bar_levels(totals: &[i64]) -> Vec<usize> {
    let max = totals.iter().copied().max().unwrap_or(0);
    totals
        .iter()
        .flat_map(|value| {
            let height = if *value <= 0 || max <= 0 {
                0
            } else {
                ((*value * DAY_COUNT as i64 + max - 1) / max) as usize
            };
            (0..DAY_COUNT).map(move |row| if DAY_COUNT - row <= height { 4 } else { 0 })
        })
        .collect()
}

fn chart_start(today: NaiveDate) -> NaiveDate {
    let week_start = today - Duration::days(i64::from(today.weekday().num_days_from_sunday()));
    week_start - Duration::weeks((WEEK_COUNT - 1) as i64)
}

fn cell_date(today: NaiveDate, index: usize) -> Option<NaiveDate> {
    chart_start(today).checked_add_signed(Duration::days(index as i64))
}

#[cfg(test)]
#[path = "tokens_chart_tests.rs"]
mod tests;
