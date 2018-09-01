use std::{
    fmt::Write,
};

use cursive::{
    Printer,
    direction::Direction,
    event::{AnyCb, Event, Key, EventResult, MouseButton, MouseEvent},
    theme::{Effect, Color, BaseColor},
    view::{View, Selector},
    vec::Vec2,
};

use mahboi::{
    log::*,
    machine::Machine,
    primitives::{Byte, Word},
};


const DATA_OFFSET: usize = 9;
const DATA_LEN: usize = 3 * 16 - 1;


pub struct MemView {
    /// Address of the first byte in the first line. Is always divisable by 16.
    first_line_addr: Word,

    /// Cached data from the memory. Always holds 16*16=256 bytes.
    data: Vec<Byte>,

    /// Position of the cursor
    pub(crate) cursor: Word,
}

impl MemView {
    /// Creates an empty MemView.
    pub fn new() -> Self {
        Self {
            first_line_addr: Word::new(0),
            data: vec![],
            cursor: Word::new(0),
        }
    }

    /// Updates the memory data and scrolling position.
    pub(crate) fn update(&mut self, machine: &Machine, state_changed: bool) {
        // Check if we need to adjust our window
        let cursor_line = self.cursor.get() & 0xFFF0;
        debug!("{} -- {}", cursor_line, self.first_line_addr);
        let needs_update = if cursor_line <= self.first_line_addr.get() {
            self.first_line_addr = Word::new(cursor_line.saturating_sub(0x10));
            true
        } else if cursor_line >= self.first_line_addr.get() + 0xE0 {
            let offset = if cursor_line == 0xFFF0 {
                0xF0
            } else {
                0xE0
            };
            self.first_line_addr = Word::new(cursor_line - offset);
            true
        } else {
            self.data.is_empty()
        };


        if state_changed || needs_update {
            self.data.clear();

            for i in 0u16..16 * 16 {
                let addr = self.first_line_addr + i;
                self.data.push(machine.load_byte(addr));
            }
        }
    }
}

impl View for MemView {
    fn draw(&self, printer: &Printer) {
        let mut buf = String::new();

        // Print header
        printer.with_style(Color::Light(BaseColor::Blue), |printer| {
            for col in 0..16 {
                buf.clear();
                write!(buf, "_{:X}", col);
                printer.print((DATA_OFFSET + 3 * col, 0), &buf);
            }

            printer.print((DATA_OFFSET - 2, 1), "┌");
            printer.print_hline((DATA_OFFSET - 1, 1), DATA_LEN + 2, "─");
        });

        // Print lines
        for (row, line) in self.data.chunks(16).enumerate() {
            // Print line start offset
            let addr = self.first_line_addr + (row as u16) * 16;
            printer.with_style(Color::Light(BaseColor::Blue), |printer| {
                buf.clear();
                write!(buf, "{} │", addr);
                printer.print((0, row + 2), &buf);
            });

            // Print actual data
            for (col, b) in line.iter().enumerate() {
                buf.clear();
                write!(buf, "{:02x}", b.get());

                let effect = if self.cursor == addr + col as u8 {
                    Effect::Reverse
                } else {
                    Effect::Simple
                };
                printer.with_effect(effect, |printer| {
                    printer.print((DATA_OFFSET + col * 3, row + 2), &buf);
                });
            }
        }

        // Print remaining border
        printer.with_style(Color::Light(BaseColor::Blue), |printer| {
            let line = 2 + 16;
            printer.print((DATA_OFFSET - 2, line), "└");
            printer.print_hline((DATA_OFFSET - 1, line), 1 + 3 * 16, "─");

            let end = DATA_OFFSET + DATA_LEN + 1;
            printer.print_vline((end, 2), 16, "│");
            printer.print((end, 1), "┐");
            printer.print((end, line), "┘");
        });
    }

    fn required_size(&mut self, _constraint: Vec2) -> Vec2 {
        Vec2::new(
            // Width: offset + seperator + 16 * (byte + space) + seperator
            DATA_OFFSET + DATA_LEN + 2,

            // Height: header + 16 lines + box border
            2 + 16 + 1,
        )
    }

    /// Reacts to arrow keys, page up and down as well as mouse click inside
    /// the data area.
    fn on_event(&mut self, event: Event) -> EventResult {
        match event {
            Event::Key(Key::Left) => {
                if self.cursor.get() % 16 != 0 {
                    self.cursor -= 1u16;
                }
                EventResult::Consumed(None)
            }
            Event::Key(Key::Right) => {
                if self.cursor.get() % 16 != 15 {
                    self.cursor += 1u16;
                }
                EventResult::Consumed(None)
            }
            Event::Key(Key::Up) => {
                if self.cursor.get() / 16 != 0 {
                    self.cursor -= 16u16;
                }
                EventResult::Consumed(None)
            }
            Event::Key(Key::Down) => {
                if self.cursor.get() / 16 != 0xFFF {
                    self.cursor += 16u16;
                }
                EventResult::Consumed(None)
            }
            Event::Key(Key::PageDown) => {
                self.cursor = self.cursor.map(|x| x.saturating_add(0x100) & 0xFFF0);
                EventResult::Consumed(None)
            }
            Event::Key(Key::PageUp) => {
                self.cursor = self.cursor.map(|x| x.saturating_sub(0x100) & 0xFFF0);
                EventResult::Consumed(None)
            }

            Event::Mouse { event: mouse_event, position, offset } => {
                if mouse_event != MouseEvent::Press(MouseButton::Left) {
                    return EventResult::Ignored;
                }

                if let Some(rel_pos) = position.checked_sub(offset) {
                    // Check if the click was inside of the data area
                    if rel_pos.y < 2 || rel_pos.y >= 18 {
                        return EventResult::Ignored;
                    }
                    if rel_pos.x < DATA_OFFSET || rel_pos.x > DATA_OFFSET + DATA_LEN {
                        return EventResult::Ignored;
                    }

                    // If the click is between two bytes, we ignore it
                    let x_inside = rel_pos.x - DATA_OFFSET;
                    if x_inside % 3 == 2 {
                        return EventResult::Ignored;
                    }

                    // Calculate byte offset
                    let col = x_inside / 3;
                    let row = rel_pos.y - 2;
                    let line_offset = self.first_line_addr + (0x10 * row as u16);
                    self.cursor = line_offset + col as u8;

                    EventResult::Consumed(None)
                } else {
                    EventResult::Ignored
                }
            }

            _ => EventResult::Ignored,
        }
    }

    fn take_focus(&mut self, _: Direction) -> bool {
        true
    }

    fn call_on_any<'a>(&mut self, _selector: &Selector, _cb: AnyCb<'a>) {}
}
