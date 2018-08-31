use crate::primitives::Byte;


/// Manages the input from the Joypad. This is mapped to 0xFF00 in the Memory.
pub(crate) struct InputController {
    // TODO: Implement Joypad Input
    // TODO: Implement Joypad Interrupt
}

impl InputController {
    /// Creates an instance with no buttons pressed and no input selected.
    pub(crate) fn new() -> Self {
        Self {}
    }

    /// Loads the input register.
    ///
    /// This function behaves like the real input register. Meaning: Bits 6 and 7 always return
    /// 1.
    pub(crate) fn load_register(&self) -> Byte {
        // TODO: Return real input
        Byte::new(0b1111_1111)
    }

    /// Stores a byte to the input register.
    ///
    /// This function behaves like the real input register. Meaning: Only Bits 5 and 4 are
    /// writable.
    pub(crate) fn store_register(&mut self, byte: Byte) {
        let select_button = (byte.get() & 0b0010_0000) == 0;
        let select_keys = (byte.get() & 0b0001_0000) == 0;

        // TODO select inputs
        match (select_button, select_keys) {
            _ => {}
        }
    }
}
