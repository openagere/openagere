use crate::color::blend;
use crate::color::is_light;
use crate::terminal_palette::best_color;
use crate::terminal_palette::default_bg;
use ratatui::style::Color;
use ratatui::style::Style;
use ratatui::text::Line;

pub fn user_message_style() -> Style {
    user_message_style_for(default_bg())
}

pub fn popup_surface_style() -> Style {
    popup_surface_style_for(default_bg())
}

pub fn proposed_plan_style() -> Style {
    proposed_plan_style_for(default_bg())
}

/// Returns the style for a user-authored message using the provided terminal background.
pub fn user_message_style_for(terminal_bg: Option<(u8, u8, u8)>) -> Style {
    match terminal_bg {
        Some(bg) => Style::default().bg(user_message_bg(bg)),
        None => Style::default(),
    }
}

pub fn popup_surface_style_for(terminal_bg: Option<(u8, u8, u8)>) -> Style {
    match terminal_bg {
        Some(bg) => Style::default().bg(user_message_bg(bg)),
        None => Style::default(),
    }
}

pub fn proposed_plan_style_for(terminal_bg: Option<(u8, u8, u8)>) -> Style {
    match terminal_bg {
        Some(bg) => Style::default().bg(proposed_plan_bg(bg)),
        None => Style::default(),
    }
}

#[allow(clippy::disallowed_methods)]
pub fn user_message_bg(terminal_bg: (u8, u8, u8)) -> Color {
    let (top, alpha) = if is_light(terminal_bg) {
        ((0, 0, 0), 0.04)
    } else {
        ((255, 255, 255), 0.12)
    };
    best_color(blend(top, terminal_bg, alpha))
}

#[allow(clippy::disallowed_methods)]
pub fn proposed_plan_bg(terminal_bg: (u8, u8, u8)) -> Color {
    user_message_bg(terminal_bg)
}

/// Force all spans in a line to green, overriding any existing span colors.
pub(crate) fn green_line(mut line: Line<'static>) -> Line<'static> {
    for span in &mut line.spans {
        span.style = span.style.fg(Color::Green);
    }
    line
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn user_message_style_has_no_fallback_background() {
        assert_eq!(user_message_style_for(None).bg, None);
    }

    #[test]
    fn user_message_style_uses_theme_aware_background_when_available() {
        let terminal_bg = (12, 34, 56);

        assert_eq!(
            user_message_style_for(Some(terminal_bg)).bg,
            Some(user_message_bg(terminal_bg))
        );
    }

    #[test]
    fn popup_surface_style_uses_fallback_background_when_terminal_bg_unknown() {
        assert_eq!(popup_surface_style_for(None).bg, None);
    }
}
