use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::style::Stylize;
use ratatui::text::Line;
use ratatui::text::Span;
use ratatui::widgets::List;
use ratatui::widgets::ListItem;
use ratatui::widgets::ListState;
use ratatui::widgets::Paragraph;
use ratatui::widgets::Widget;
use ratatui::widgets::WidgetRef;

use crate::external_rg::RgMatch;

#[derive(Default)]
pub(crate) struct GrepSearchPopup {
    display_query: String,
    matches: Vec<RgMatch>,
    list_state: ListState,
    waiting: bool,
}

impl GrepSearchPopup {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    pub(crate) fn set_query(&mut self, query: String) {
        self.display_query = query;
        self.matches.clear();
        self.list_state.select(Some(0));
        self.waiting = true;
    }

    pub(crate) fn set_matches(&mut self, matches: Vec<RgMatch>) {
        self.matches = matches;
        self.waiting = false;
        if self.matches.is_empty() {
            self.list_state.select(None);
        } else {
            self.list_state.select(Some(0));
        }
    }

    pub(crate) fn move_up(&mut self) {
        if self.matches.is_empty() {
            return;
        }
        let len = self.matches.len();
        let current = self.list_state.selected().unwrap_or(0);
        let next = if current == 0 { len - 1 } else { current - 1 };
        self.list_state.select(Some(next));
    }

    pub(crate) fn move_down(&mut self) {
        if self.matches.is_empty() {
            return;
        }
        let len = self.matches.len();
        let current = self.list_state.selected().unwrap_or(0);
        let next = if current + 1 >= len { 0 } else { current + 1 };
        self.list_state.select(Some(next));
    }

    pub(crate) fn selected_match(&self) -> Option<&RgMatch> {
        self.list_state.selected().and_then(|i| self.matches.get(i))
    }

    pub(crate) fn calculate_required_height(&self) -> u16 {
        let items = if self.waiting {
            1
        } else if self.matches.is_empty() {
            1
        } else {
            self.matches.len().min(10)
        };
        (items as u16) + 2
    }
}

impl WidgetRef for GrepSearchPopup {
    fn render_ref(&self, area: Rect, buf: &mut Buffer) {
        let inner = area.inner(ratatui::layout::Margin::new(1, 1));

        if self.waiting {
            let text = format!("Searching for '{}'...", self.display_query);
            let paragraph = Paragraph::new(Line::from(Span::raw(text)));
            paragraph.render(inner, buf);
            return;
        }

        if self.matches.is_empty() {
            let paragraph = Paragraph::new(Line::from(Span::raw("No matches found")));
            paragraph.render(inner, buf);
            return;
        }

        let items: Vec<ListItem> = self
            .matches
            .iter()
            .take(10)
            .map(|m| {
                let text = format!("{}:{}: {}", m.path, m.line_number, m.line);
                ListItem::new(Line::from(Span::raw(text)))
            })
            .collect();

        let list = List::new(items).highlight_style(Style::new().reversed());
        list.render_ref(inner, buf);
    }
}
