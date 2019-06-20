use bit_field::BitField;

use crate::{
    primitives::Byte,
    env::Input,
    machine::interrupt::InterruptController,
};


/// Manages the input from the Joypad. This is mapped to 0xFF00 in the Memory.
pub(crate) struct InputController {
    // TODO: Implement Joypad Interrupt
    register: Byte,
}

impl InputController {
    /// Creates an instance with no buttons pressed and no input selected.
    pub(crate) fn new() -> Self {
        Self {
            register: Byte::new(0xFF),
        }
    }

    /// Loads the input register.
    ///
    /// This function behaves like the real input register. Meaning: Bits 6 and 7 always return
    /// 1.
    pub(crate) fn load_register(&self) -> Byte {
        self.register.map(|b| b | 0b1100_0000)
    }

    /// Stores a byte to the input register.
    ///
    /// This function behaves like the real input register. Meaning: Only Bits 5 and 4 are
    /// writable.
    pub(crate) fn store_register(&mut self, byte: Byte) {
        let mask = 0b0011_0000;
        self.register = byte.map(|b| b & mask) | self.register.map(|b| b & !mask);
    }

    /// Reacts to the input transmitted via the input parameter.
    pub(crate) fn handle_input(
        &mut self,
        input: &impl Input,
        _interrupt_controller: &mut InterruptController,
    ) {
        let pressed = input.get_pressed_keys();
        let keys = match (self.is_direction_selected(), self.is_button_selected()) {
            (false, false) => 0,
            (false, true) => pressed.get_button_keys(),
            (true, false) => pressed.get_direction_keys(),
            (true, true) => pressed.get_direction_keys() | pressed.get_button_keys(),
        };

        self.register = self.register.map(|r| {
            (r & 0b1111_0000) | (!keys & 0b0000_1111)
        });
    }

    /// Returns true, if the button keys are selected, false otherwise.
    fn is_button_selected(&self) -> bool {
        (self.register.get() & 0b0010_0000) == 0
    }

    /// Returns true, if the direction keys are selected, false otherwise.
    fn is_direction_selected(&self) -> bool {
        (self.register.get() & 0b0001_0000) == 0
    }
}

/// Represents the buttons pressed on the Joypad in an easy and convenient way (some people say,
/// Nintedo should have implemented their Joypad register this way). The bits in this u8
/// represent the buttons with 0: not pressed and 1: pressed. The relation is:
/// - 0: A    (LSB)
/// - 1: B
/// - 2: Select
/// - 3: Start
/// - 4: Right
/// - 5: Left
/// - 6: Up
/// - 7: Down (MSB)
#[derive(Clone, Copy, Debug)]
pub struct Keys(u8);

impl Keys {
    /// Creates an instance with no buttons pressed.
    pub fn none() -> Self {
        Keys(0x00)
    }

    /// Sets the given key in this instance to the given state.
    pub fn set_key(mut self, key: JoypadKey, is_pressed: bool) -> Self {
        let bit = match key {
            JoypadKey::A => 0,
            JoypadKey::B => 1,
            JoypadKey::Select => 2,
            JoypadKey::Start => 3,
            JoypadKey::Right => 4,
            JoypadKey::Left => 5,
            JoypadKey::Up => 6,
            JoypadKey::Down => 7,
        };
        self.0.set_bit(bit, is_pressed);
        self
    }

    /// Returns the direction keys in the low nybble (the high nybble is 0).
    pub(crate) fn get_direction_keys(&self) -> u8 {
        (self.0 >> 4) & 0x0F
    }

    /// Returns the button keys in the low nybble (the high nybble is 0).
    pub(crate) fn get_button_keys(&self) -> u8 {
        self.0 & 0x0F
    }
}

/// Represents a key on the Game Boy.
#[derive(Clone, Copy, Debug)]
pub enum JoypadKey {
    A,
    B,
    Start,
    Select,
    Up,
    Right,
    Down,
    Left,
}

#[cfg(test)]
mod test {
    use super::*;


    struct DummyInput {
        keys: Vec<JoypadKey>,
    }

    impl Input for DummyInput {
        fn get_pressed_keys(&self) -> Keys {
            let mut out = Keys::none();
            for &key in &self.keys {
                out = out.set_key(key, true);
            }
            out
        }
    }

    #[test]
    fn test_input_controller_handle_input() {
        fn run(keys: Vec<JoypadKey>, byte: u8) -> Byte {
            let mut ic = InputController::new();
            let mut ih = InterruptController::new();
            let dummy_input = DummyInput {
                keys,
            };
            ic.store_register(Byte::new(byte));
            ic.handle_input(&dummy_input, &mut ih);
            ic.load_register()
        }

        // None selected
        assert_eq!(run(vec![], 0b1011_0011), 0b1111_1111);
        assert_eq!(run(vec![JoypadKey::A], 0b0011_1111), 0b1111_1111);
        assert_eq!(run(vec![JoypadKey::A, JoypadKey::Up], 0b0011_0000), 0b1111_1111);

        // Buttons selected
        assert_eq!(run(vec![], 0b1101_1101), 0b1101_1111);
        assert_eq!(run(vec![JoypadKey::Left], 0b1001_0010), 0b1101_1111);
        assert_eq!(run(vec![JoypadKey::A], 0b1001_0010), 0b1101_1110);
        assert_eq!(run(vec![JoypadKey::A, JoypadKey::Up], 0b0001_0011), 0b1101_1110);
        assert_eq!(
            run(vec![JoypadKey::A, JoypadKey::Up, JoypadKey::Start], 0b0001_0000),
            0b1101_0110,
        );

        // Directions selected
        assert_eq!(run(vec![], 0b1110_1101), 0b1110_1111);
        assert_eq!(run(vec![JoypadKey::Left], 0b1010_0110), 0b1110_1101);
        assert_eq!(run(vec![JoypadKey::A], 0b1010_0010), 0b1110_1111);
        assert_eq!(run(vec![JoypadKey::A, JoypadKey::Up], 0b0010_0011), 0b1110_1011);
        assert_eq!(
            run(
                vec![JoypadKey::A, JoypadKey::Right, JoypadKey::Start, JoypadKey::Up],
                0b0010_0000,
            ),
            0b1110_1010,
        );

        // Both selected
        assert_eq!(run(vec![], 0b1100_1101), 0b1100_1111);
        assert_eq!(run(vec![JoypadKey::Left], 0b1000_0110), 0b1100_1101);
        assert_eq!(run(vec![JoypadKey::A], 0b1000_0010), 0b1100_1110);
        assert_eq!(run(vec![JoypadKey::A, JoypadKey::Up], 0b0000_0011), 0b1100_1010);
        assert_eq!(
            run(
                vec![JoypadKey::A, JoypadKey::Right, JoypadKey::Start, JoypadKey::Up],
                0b0000_0000,
            ),
            0b1100_0010,
        );
        assert_eq!(
            run(
                vec![JoypadKey::B, JoypadKey::Right, JoypadKey::Start, JoypadKey::Up],
                0b0000_0000,
            ),
            0b1100_0000,
        );
    }
}
