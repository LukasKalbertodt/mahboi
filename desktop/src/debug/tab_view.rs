use cursive::{
    Printer, With, Cursive,
    align::{Align, HAlign, VAlign},
    direction::Direction,
    event::{Callback, Event, EventResult, Key, MouseButton, MouseEvent},
    menu::MenuTree,
    rect::Rect,
    theme::{ColorStyle, Effect},
    view::{Position, View},
    views::MenuPopup,
    vec::Vec2,
};
use unicode_width::UnicodeWidthStr;



pub struct TabView {
    tabs: Vec<String>,
    selected_tab: u8,
}

impl TabView {
    /// Creates a new empty TabView.
    pub fn new(tabs: Vec<impl Into<String>>) -> Self {
        assert!(tabs.len() != 0);

        Self {
            tabs: tabs.into_iter().map(Into::into).collect(),
            selected_tab: 0,
        }
    }

    /// Returns the id of the item currently selected.
    ///
    /// Returns `None` if the list is empty.
    pub fn selected_tab(&self) -> u8 {
        self.selected_tab
    }

    pub fn len(&self) -> u8 {
        self.tabs.len() as u8
    }

    /// Selects the tab left of the currently selected tab. Does nothing if the
    /// leftmost tab is already selected.
    pub fn select_left(&mut self) {
        if self.selected_tab > 0 {
            self.selected_tab -= 1;
        }
    }

    /// Selects the tab right of the currently selected tab. Does nothing if the
    /// rightmost tab is already selected.
    pub fn select_right(&mut self) {
        if self.selected_tab < self.len() - 1 {
            self.selected_tab += 1;
        }
    }
}

impl View for TabView {
    fn draw(&self, printer: &Printer) {
        let mut offset = 0;
        for (i, tab_title) in self.tabs.iter().enumerate() {
            let width = tab_title.width();
            printer.print((offset, 0), " ");

            printer.with_selection(i == self.selected_tab as usize, |printer| {
                printer.print((offset + 1, 0), tab_title);
            });
            printer.print((offset + 1 + width, 0), " ");
            printer.print((offset + 1 + width + 1, 0), "│");

            printer.print_hline((offset, 1), width + 2, "─");
            printer.print_hline((offset + width + 2, 1), 1, "┴");

            offset += width + 3;
        }

        printer.print_hline((offset, 1), printer.size.x.saturating_sub(offset), "─");
    }

    fn required_size(&mut self, _: Vec2) -> Vec2 {
        // The tab bar is not compressible.
        let w = self.tabs.iter().map(|s| s.width()).sum();

        Vec2::new(w, 2)
    }

    fn on_event(&mut self, event: Event) -> EventResult {
        match event {
            Event::Key(Key::PageUp) => self.select_left(),
            Event::Key(Key::PageDown) => self.select_right(),

            Event::Mouse {
                event: MouseEvent::Press(_),
                position,
                offset,
            } => {
                if let Some(rel_pos) = position.checked_sub(offset) {
                    if rel_pos.y != 0 {
                        return EventResult::Ignored;
                    }

                    let mut offset = 0;
                    for (i, tab_title) in self.tabs.iter().enumerate() {
                        let end = offset + tab_title.width() + 2;
                        if rel_pos.x >= offset && rel_pos.x < end {
                            self.selected_tab = i as u8;
                            break;
                        }

                        offset = end + 1;
                    }
                }
            }
            _ => return EventResult::Ignored,
        }

        EventResult::Consumed(None)
    }

    fn take_focus(&mut self, _: Direction) -> bool {
        true
    }
}
