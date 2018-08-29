use std::collections::VecDeque;

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

use super::LOG_MESSAGES;


/// Determines how many log messages are drawn at the same time. Of course, not
/// all messages are on the screen, because this log view is in a scroll view.
/// However, showing a lot of entries makes the TUI very slow.
const MAX_ENTRIES_IN_VIEW: usize = 100;

struct Entry {
    level: Level,
    text: TextView,

    /// Cached height. Is 0 before `required_size` was called.
    height: usize,
}

pub struct LogView {
    /// The entries we currently show in the view.
    entries: VecDeque<Entry>,

    /// The length of the global `LOG_MESSAGES` when we last checked
    last_global_len: usize,
}

impl LogView {
    /// Creates an empty LogView.
    pub fn new() -> Self {
        Self {
            entries: VecDeque::new(),
            last_global_len: 0,
        }
    }

    /// Updates the view and pulls the newest messages from the global buffer.
    pub(crate) fn update(&mut self) {
        let global_logs = LOG_MESSAGES.lock().unwrap();

        // If new messages were added,
        if global_logs.len() > self.last_global_len {
            let num_new_entries = global_logs.len() - self.last_global_len;

            // If we would have too many entries, we will remove a few from the
            // list.
            let pop_count = (self.entries.len() + num_new_entries)
                .saturating_sub(MAX_ENTRIES_IN_VIEW);
            for _ in 0..pop_count {
                self.entries.pop_front();
            }

            // Add new entries
            for record in &global_logs[self.last_global_len..] {
                // Prepare view. We disable content wrap for log messages of
                // level `Trace`, because calculating text wrap is costly.
                let mut view = TextView::new(record.msg.clone());
                view.set_content_wrap(record.level != log::Level::Trace);

                self.entries.push_back(Entry {
                    level: record.level,
                    text: view ,
                    height: 0,
                })
            }

            self.last_global_len = global_logs.len();
        }
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
        let constraint_for_child = Vec2::new(constraint.x - 7, constraint.y);
        let mut height = 0;
        for entry in &mut self.entries {
            entry.height = entry.text.required_size(constraint_for_child).y;
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
