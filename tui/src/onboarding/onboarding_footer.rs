//! A thin footer bar for the onboarding screens.
//!
//! Rendered at the bottom of the screen with a fixed height so that the step
//! content area above it can flex-expand and the footer stays pinned during
//! terminal resize.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Stylize;
use ratatui::text::Line;
use ratatui::text::Span;
use ratatui::widgets::WidgetRef;
use unicode_width::UnicodeWidthStr;

/// Fixed height of the footer (hint line + 1 blank spacer).
const FOOTER_HEIGHT: u16 = 2;

pub(crate) struct OnboardingFooter<'a> {
    hints: Vec<Span<'a>>,
}

impl<'a> OnboardingFooter<'a> {
    pub(crate) fn new(hints: Vec<Span<'a>>) -> Self {
        Self { hints }
    }

    pub(crate) fn desired_height(&self) -> u16 {
        FOOTER_HEIGHT
    }
}

impl WidgetRef for OnboardingFooter<'_> {
    fn render_ref(&self, area: Rect, buf: &mut Buffer) {
        let bottom = area.y.saturating_add(area.height);
        let inner_x = area.x.saturating_add(2);
        let inner_w = area.width.saturating_sub(4);
        let line_area = |y: u16| Rect {
            x: inner_x,
            y,
            width: inner_w,
            height: 1,
        };

        let mut y = area.y;

        // Spacer row.
        if y < bottom {
            y = y.saturating_add(1);
        }

        // Hints line.
        if y < bottom && !self.hints.is_empty() {
            let line: Line<'_> = Line::from(self.hints.clone());
            let line_width = line
                .iter()
                .map(|span| UnicodeWidthStr::width(span.content.as_ref()))
                .sum::<usize>() as u16;
            let area = line_area(y);
            let x = area.x.saturating_add(area.width.saturating_sub(line_width));
            line.dim().render_ref(Rect { x, ..area }, buf);
        }
    }
}

/// Split the given area into a flex-expandable content region and a fixed
/// footer. Returns `(content_area, footer_area)`.
pub(crate) fn split_onboarding_area(area: Rect) -> (Rect, Rect) {
    let total_h = area.height;
    let footer_h = FOOTER_HEIGHT.min(total_h);
    let content_h = total_h.saturating_sub(footer_h);

    let content = Rect {
        x: area.x,
        y: area.y,
        width: area.width,
        height: content_h,
    };
    let footer = Rect {
        x: area.x,
        y: area.y.saturating_add(content_h),
        width: area.width,
        height: footer_h,
    };
    (content, footer)
}
