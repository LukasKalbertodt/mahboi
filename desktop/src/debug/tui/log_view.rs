use std::cmp;

use cursive::{
    Printer,
    direction::Direction,
    event::{AnyCb, Event, EventResult},
    theme::{ColorStyle, Color, ColorType, BaseColor},
    view::{View, Selector},
    views::TextView,
    vec::Vec2,
};
use log::Level;


struct Entry {
    level: Level,
    text: TextView,
    height: usize,
}

pub struct LogView {
    entries: Vec<Entry>,
    height: usize,
}

impl LogView {
    /// Creates an empty LogView.
    pub fn new() -> Self {
        Self {
            entries: vec![],
            height: 0,
        }
    }

    /// Adds a tab to the tab view.
    pub fn add_row(&mut self, level: Level, msg: String) {
        let mut text = TextView::new(msg);

        let height = text.required_size(Vec2::max_value()).y;
        self.height += height;

        self.entries.push(Entry {
            level,
            text,
            height,
        });
    }
}

impl View for LogView {
    fn draw(&self, printer: &Printer) {
        fn level_to_color(level: Level) -> ColorStyle {
            let color = match level {
                Level::Trace => Color::Dark(BaseColor::White),
                Level::Debug => Color::Light(BaseColor::White),
                Level::Info => Color::Light(BaseColor::Blue),
                Level::Warn => Color::Light(BaseColor::Yellow),
                Level::Error => Color::Dark(BaseColor::Red),
            };

            ColorStyle {
                front: ColorType::Color(color),
                // back: ColorType::Palette(PaletteColor::View),
                back: ColorType::Color(Color::TerminalDefault),
            }
        }


        let mut y_offset = 0;
        for entry in &self.entries {
            let color = level_to_color(entry.level);
            printer.offset((0, y_offset)).with_color(color, |printer| {
                let lvl = format!("{:6} ", entry.level);
                printer.print((0, 0), &lvl);

                entry.text.draw(&printer.offset((7, 0)));
            });
            y_offset += entry.height;
        }
    }

    fn required_size(&mut self, constraint: Vec2) -> Vec2 {
        let height = cmp::max(constraint.y, self.height);

        Vec2::new(constraint.x, height)
    }

    fn on_event(&mut self, _: Event) -> EventResult {
        EventResult::Ignored
    }

    fn take_focus(&mut self, _: Direction) -> bool {
        true
    }

    fn call_on_any<'a>(&mut self, _selector: &Selector, _cb: AnyCb<'a>) {
        // TODO
    }
}
