//! Popup surface utility for TUI visual redesign.
//!
//! Provides pre-configured ratatui `Block` widgets for popup surfaces.

use ratatui::style::Style;
use ratatui::widgets::Block;

/// Neon border style variants.
#[derive(Clone, Copy, Debug)]
pub(crate) enum NeonStyle {
    /// Gray border for popups / modals.
    Popup,
}

impl NeonStyle {
    fn style(self) -> Style {
        Style::default()
    }
}

/// Returns a pre-configured `Block` for popup surfaces.
///
/// Callers can further customize the returned block (e.g., `.title()`) before rendering.
pub(crate) fn neon_frame(style: NeonStyle) -> Block<'static> {
    Block::new().style(style.style())
}

/// Returns a popup surface block without a border.
pub(crate) fn popup_frame() -> Block<'static> {
    neon_frame(NeonStyle::Popup)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::buffer::Buffer;
    use ratatui::layout::Rect;
    use ratatui::widgets::Widget;

    #[test]
    fn popup_frame_has_no_border_glyphs() {
        let area = Rect::new(0, 0, 12, 3);
        let mut buf = Buffer::empty(area);

        popup_frame().render(area, &mut buf);

        for y in area.y..area.bottom() {
            for x in area.x..area.right() {
                assert_eq!(buf[(x, y)].symbol(), " ");
            }
        }
    }
}
