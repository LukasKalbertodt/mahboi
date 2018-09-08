use std::collections::VecDeque;

use cursive::{
    Cursive, Printer,
    direction::Direction,
    event::{AnyCb, Event, EventResult},
    theme::{ColorStyle, Color, ColorType, BaseColor},
    view::{View, Selector, Scrollable, ScrollStrategy, Identifiable},
    views::{RadioGroup, LinearLayout, Dialog, TextView, Checkbox},
    vec::Vec2,
};
use log::{Level, LevelFilter};

use super::{LOG_MESSAGES, LogMessage};


/// Determines how many log messages are drawn at the same time. Of course, not
/// all messages are on the screen, because this log view is in a scroll view.
/// However, showing a lot of entries makes the TUI very slow.
const MAX_ENTRIES_IN_VIEW: usize = 1000;

struct Entry {
    level: Level,
    // text: TextView,
    text: String,

    /// Cached height (number of `\n` + 1)
    height: usize,
}

impl Entry {
    fn new(record: &LogMessage) -> Self {
        Self {
            level: record.level,
            text: record.msg.clone(),
            height: record.msg.lines().count(),
        }
    }
}

pub struct LogView {
    /// The entries we currently show in the view.
    entries: VecDeque<Entry>,

    /// The radio group representing the dialog to filter log messages.
    filter: RadioGroup<LevelFilter>,

    /// The length of the global `LOG_MESSAGES` when we last checked
    last_global_len: usize,

    last_filter_level: LevelFilter,
}

impl LogView {
    /// Creates an empty LogView.
    pub fn new() -> LinearLayout {
        let mut radio_group = RadioGroup::new();
        let log_level_box = LinearLayout::vertical()
            .child(radio_group.button(LevelFilter::Trace, "Trace"))
            .child(radio_group.button(LevelFilter::Debug, "Debug"))
            .child(radio_group.button(LevelFilter::Info, "Info"))
            .child(radio_group.button(LevelFilter::Warn, "Warn"))
            .child(radio_group.button(LevelFilter::Error, "Error"));

        let log_level_box = Dialog::around(log_level_box)
            .title("Filter Logs");

        let options_box = LinearLayout::vertical()
            .child(Checkbox::new().checked().with_id("ignore_trace_box"))
            .child(TextView::new("ignore TRACE while running"));

        let options_box = Dialog::around(options_box)
            .title("Options");

        let right_panel = LinearLayout::vertical()
            .child(log_level_box)
            .child(options_box);

        // Create the list showing the log messages
        let log_list = Self {
            entries: VecDeque::new(),
            filter: radio_group,
            last_global_len: 0,
            last_filter_level: LevelFilter::Trace,
        };
        let log_list = log_list
            .with_id("log_list")
            .scrollable()
            .scroll_strategy(ScrollStrategy::StickToBottom);

        LinearLayout::horizontal()
            .child(log_list)
            .child(right_panel)
    }

    pub(crate) fn ignore_trace_logs(&self, siv: &mut Cursive) -> bool {
        siv.find_id::<Checkbox>("ignore_trace_box").unwrap().is_checked()
    }

    /// Updates the view and pulls the newest messages from the global buffer.
    pub(crate) fn update(&mut self) {
        let global_logs = LOG_MESSAGES.lock().unwrap();

        // If the filter was changed, we need to update out whole buffer.
        if self.last_filter_level != *self.filter.selection() {
            let filter = *self.filter.selection();
            self.entries.clear();

            // Select the last `MAX_ENTRIES_IN_VIEW` many entries which satisfy
            // the filter.
            let records_rev = global_logs.iter()
                .rev()
                .filter(|e| e.level <= filter)
                .take(MAX_ENTRIES_IN_VIEW);

            // Add them to our buffer (`push_front` because the iterator is
            // reversed).
            for record in records_rev {
                self.entries.push_front(Entry::new(record));
            }

            // Update cache
            self.last_filter_level = filter;
            self.last_global_len = global_logs.len();
        }

        // If new messages were added, we need to potentially add them.
        if global_logs.len() > self.last_global_len {
            // See how many of the new messages we actually need to display.
            let filter = self.last_filter_level;
            let new_entries = global_logs[self.last_global_len..].iter()
                .filter(|e| e.level <= filter);
            let num_new_entries = new_entries.clone().count();

            // If we would have too many entries, we will remove a few from the
            // list.
            let pop_count = (self.entries.len() + num_new_entries)
                .saturating_sub(MAX_ENTRIES_IN_VIEW);
            for _ in 0..pop_count {
                self.entries.pop_front();
            }

            // Add new entries
            for record in new_entries {
                self.entries.push_back(Entry::new(record));
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
            printer.with_color(color, |printer| {
                let lvl = format!("{:6} ", entry.level);
                printer.print((0, y_offset), &lvl);

                // entry.text.draw(&printer.offset((7, 0)));
                for line in entry.text.lines() {
                    printer.print((7, y_offset), line);
                    y_offset += 1;
                }
            });
        }
    }

    fn required_size(&mut self, constraint: Vec2) -> Vec2 {
        let height = self.entries.iter().map(|e| e.height).sum();
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
