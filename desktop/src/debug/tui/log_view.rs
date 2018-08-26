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

    /// Cached height. Is 0 before `required_size` was called.
    height: usize,
}

pub struct LogView {
    entries: Vec<Entry>,
}

impl LogView {
    /// Creates an empty LogView.
    pub fn new() -> Self {
        Self {
            entries: vec![],
        }
    }

    /// Adds a tab to the tab view.
    pub fn add_row(&mut self, level: Level, msg: String) {
        self.entries.push(Entry {
            level,
            text: TextView::new(msg),
            height: 0,
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
                back: ColorType::Color(Color::TerminalDefault),
            }
        }

        // We do some cheap scrolling here: if the available size is less than
        // we need, we simply don't draw the entries that don't start on the
        // screen. This should be replaced with a `ScrollView`, but
        // unfortunately it's at the moment...
        let mut y_offset = 0;
        for entry in &self.entries {
            let color = level_to_color(entry.level);
            printer.offset((0, y_offset)).with_color(color, |printer| {
                let lvl = format!("{:6} ", entry.level);
                printer.print((0, 0), &lvl);

                entry.text.draw(&printer.offset((7, 0)));
            });

            y_offset += entry.height as i32;
        }
    }

    fn required_size(&mut self, constraint: Vec2) -> Vec2 {
        let mut height = 0;
        for entry in &mut self.entries {
            entry.height = entry.text.required_size(constraint).y;
            height += entry.height;
        }

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
