use crate::{
    primitives::Word,
};

pub trait Peripherals {
    type Display: Display;
    type Sound: Sound;
    type Input: Input;

    fn display(&mut self) -> &mut Self::Display;
    fn sound(&mut self) -> &mut Self::Sound;
    fn input(&mut self) -> &mut Self::Input;
}

pub trait Display {
//    fn set_pixel(...);
}

pub trait Sound {

}

pub trait Input {

}

pub struct PixelPos {
    x: u8,
    y: u8,
}

impl PixelPos {
    /// Creates a new `PixelPos`. `x` has to be between 0 and 159 (inclusive)
    /// and `y` has to be between 0 and 143 (inclusive).
    pub fn new(x: u8, y: u8) -> Self {
        assert!(x < 160);
        assert!(y < 144);

        Self { x, y }
    }

    pub fn x(&self) -> u8 {
        self.x
    }

    pub fn y(&self) -> u8 {
        self.y
    }
}

/// A gameboy color pixel color.
///
/// Each channel has a depth of 5 bit = 32 different values, so `r`, `g` and
/// `b` hold values between 0 and 31 (inclusive). In sum, this means we have
/// 32^3 = 32768 different colors.
pub struct PixelColor {
    r: u8,
    g: u8,
    b: u8,
}

impl PixelColor {
    /// Decodes the color in the word, which is encoded like this:
    ///
    /// - Bit 0 - 4: Red
    /// - Bit 5 - 9: Green
    /// - Bit 10 - 14: Blue
    /// - Bit 15: not used
    pub fn from_color_word(w: Word) -> Self {
        Self {
            r: ((w.get() >>  0) as u8) & 0b0001_1111,
            g: ((w.get() >>  5) as u8) & 0b0001_1111,
            b: ((w.get() >> 10) as u8) & 0b0001_1111,
        }
    }

    /// Creates a new `PixelColor` instance. `r`, `g` and `b` have to be
    /// smaller than 32!
    pub fn new(r: u8, g: u8, b: u8) -> Self {
        assert!(r <= 31);
        assert!(g <= 31);
        assert!(b <= 31);

        Self { r, g, b }
    }

    /// Converts this color into the SRGB 24-bit color space. Returns the array
    /// `[r, g, b]`.
    ///
    /// **Note**: this function currently doesn't perform the correct
    /// conversion!
    pub fn to_srgb(&self) -> [u8; 3] {
        // TODO: well, it seems to be rather complicated
        [self.r << 3, self.g << 3, self.b << 3]
    }
}
