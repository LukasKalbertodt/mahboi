use std::cmp;

use cursive::{
    Printer, XY,
    direction::Direction,
    event::{AnyCb, Event, MouseButton, EventResult, Key, MouseEvent},
    theme::{ColorStyle, Effect, Color, BaseColor, Style},
    view::{View, Selector},
    vec::Vec2,
};
use unicode_width::UnicodeWidthStr;

use mahboi::log::*;



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

    pub fn set_selected(&mut self, index: u8) {
        assert!(index < self.len());

        self.selected_tab = index;
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
        // Draw the tab bar
        let mut offset = 0;
        for (i, tab) in self.tabs.iter().enumerate() {
            let width = tab.title.width();

            // Select style and color for the tab, depending on whether or not
            // it's selected.
            let (style, color) = if i == self.selected_tab as usize {
                (
                    Style::from(Effect::Bold).combine(Effect::Underline),
                    ColorStyle {
                        front: Color::Light(BaseColor::Green).into(),
                        back: Color::Rgb(0, 0, 0).into(),
                    },
                )
            } else {
                (Style::none(), ColorStyle::primary())
            };

            // Print padded tab title
            printer.with_color(color, |printer| {
                printer.print((offset, 0), " ");


                printer.with_style(style, |printer| {
                    printer.print((offset + 1, 0), &tab.title);
                });

                printer.print((offset + 1 + width, 0), " ");
            });

            // Print separator
            printer.print((offset + 1 + width + 1, 0), "│");

            // Print the border on the line underneath
            printer.print_hline((offset, 1), width + 2, "─");
            printer.print_hline((offset + width + 2, 1), 1, "┴");

            offset += width + 3;
        }

        // Draw a line to fill the remaining space
        printer.print_hline((offset, 1), printer.size.x.saturating_sub(offset), "─");

        // Draw the body
        self.selected().body.draw(&printer.offset((0, 2)));
    }

    fn layout(&mut self, mut size: Vec2) {
        // We need two lines for the tab bar. The rest is for the body.
        size.y -= 2;
        self.selected_mut().body.layout(size);
    }

    fn required_size(&mut self, constraint: Vec2) -> Vec2 {
        // The tab bar
        let min_width = self.tabs.iter().map(|t| t.title_width()).sum::<usize>() - 1;
        let bar_width = cmp::max(min_width, constraint.x);

        let new_constraint = Vec2::new(bar_width, constraint.y);
        let min_body_size = self.selected_mut().body.required_size(new_constraint);

        let width = cmp::max(min_body_size.x, bar_width);
        let height = cmp::max(min_body_size.y + 1, constraint.y);

        Vec2::new(width, height)
    }

    fn on_event(&mut self, event: Event) -> EventResult {
        match event {
            // We eat PageUp and PageDown events to control the tabs.
            Event::Key(Key::PageUp) => self.select_left(),
            Event::Key(Key::PageDown) => self.select_right(),

            // For mouse events, we need to check where the event happened.
            Event::Mouse { event: mouse_event, position, offset } => {
                let is_left_click = mouse_event == MouseEvent::Press(MouseButton::Left);
                match position.checked_sub(offset) {
                    // If the tab bar was clicked, this can select a new tab
                    Some(XY { x, y: 0 }) if is_left_click => {
                        let mut offset = 0;
                        for (i, tab) in self.tabs.iter().enumerate() {
                            let end = offset + tab.title.width() + 2;
                            if x >= offset && x < end {
                                self.selected_tab = i as u8;
                                break;
                            }

                            offset = end + 1;
                        }
                    }

                    // If some other mouse event happened that was not over the
                    // child, we ignore it
                    Some(XY { y, .. }) if y <= 1 => return EventResult::Ignored,

                    // Other events are forwarded
                    Some(_) => return self.selected_mut().body.on_event(event),

                    // Except if the event is not over us at all
                    None => return EventResult::Ignored,
                }
            }

            // All other events are simply forwarded to the active tab
            _ => return self.selected_mut().body.on_event(event),
        }

        EventResult::Consumed(None)
    }

    fn take_focus(&mut self, _: Direction) -> bool {
        true
    }

    fn call_on_any<'a>(&mut self, selector: &Selector, mut cb: AnyCb<'a>) {
        for tab in &mut self.tabs {
            tab.body.call_on_any(selector, Box::new(|any| cb(any)));
        }
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
