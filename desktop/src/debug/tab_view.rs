use std::cmp;

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
    tabs: Vec<Tab>,
    selected_tab: u8,
}

impl TabView {
    /// Creates an empty TabView. Make sure to add at least one tab before
    /// using this view!
    pub fn new() -> Self {
        Self {
            tabs: vec![],
            selected_tab: 0,
        }
    }

    /// Adds a tab to the tab view.
    pub fn tab(mut self, title: impl Into<String>, body: impl View) -> Self {
        self.tabs.push(Tab {
            title: title.into(),
            body: Box::new(body),
        });
        self
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

    fn selected(&self) -> &Tab {
        &self.tabs[self.selected_tab as usize]
    }

    fn selected_mut(&mut self) -> &mut Tab {
        &mut self.tabs[self.selected_tab as usize]
    }
}

impl View for TabView {
    fn draw(&self, printer: &Printer) {
        let mut offset = 0;
        for (i, tab) in self.tabs.iter().enumerate() {
            let width = tab.title.width();
            printer.print((offset, 0), " ");

            printer.with_selection(i == self.selected_tab as usize, |printer| {
                printer.print((offset + 1, 0), &tab.title);
            });
            printer.print((offset + 1 + width, 0), " ");
            printer.print((offset + 1 + width + 1, 0), "│");

            printer.print_hline((offset, 1), width + 2, "─");
            printer.print_hline((offset + width + 2, 1), 1, "┴");

            offset += width + 3;
        }

        printer.print_hline((offset, 1), printer.size.x.saturating_sub(offset), "─");

        self.selected().body.draw(&printer.offset((0, 2)));
    }

    fn layout(&mut self, mut size: Vec2) {
        // We need two lines for the tab bar. The rest is for the body.
        size.y -= 2;

        self.selected_mut().body.layout(size);
    }

    fn required_size(&mut self, constraint: Vec2) -> Vec2 {
        // The tab bar is not compressible.
        let min_width = self.tabs.iter().map(|t| t.title_width()).sum::<usize>() - 1;
        let width = cmp::max(min_width, constraint.x);

        let new_constraint = Vec2::new(width, constraint.y);
        let min_height = 2 + self.selected_mut().body.required_size(new_constraint).y;
        let height = cmp::max(min_height, constraint.y);

        Vec2::new(width, height)
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
                    for (i, tab) in self.tabs.iter().enumerate() {
                        let end = offset + tab.title.width() + 2;
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

struct Tab {
    title: String,
    body: Box<View>,
}

impl Tab {
    fn title_width(&self) -> usize {
        self.title.width() + 3
    }
}
