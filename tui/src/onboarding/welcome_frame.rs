use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::prelude::Widget;
use ratatui::style::Style;
use ratatui::style::Stylize;
use ratatui::text::Line;
use ratatui::widgets::Block;
use ratatui::widgets::Borders;
pub(crate) const HEADER_HEIGHT: u16 = 1;

pub(crate) fn inner_area(area: Rect) -> Rect {
    Rect {
        x: area.x.saturating_add(3),
        y: area.y.saturating_add(1),
        width: area.width.saturating_sub(6),
        height: area.height.saturating_sub(2),
    }
}

pub(crate) fn render(area: Rect, buf: &mut Buffer) -> Rect {
    Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().dim())
        .title(Line::from(vec![
            " ".into(),
            "✦ OpenAgere".green().bold(),
            " ".into(),
        ]))
        .render(area, buf);

    inner_area(area)
}

pub(crate) fn render_header(start_y: u16, bottom: u16) -> u16 {
    let mut y = start_y;
    if y < bottom {
        y = y.saturating_add(1);
    }
    y
}
