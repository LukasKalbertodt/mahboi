//! Types to represent Gameboy data.

use std::{
    ops::{Add, Sub, Index, IndexMut, AddAssign, SubAssign, Range},
    fmt::{self, Debug, Display},
};

use derive_more::{BitXor, BitXorAssign, Display, BitAnd, BitAndAssign, BitOr, BitOrAssign};


/// A single Gameboy byte.
///
/// This wrapper type is used to assert correct overflow behavior in arithmetic
/// operations.
#[derive(
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    BitXor,
    BitXorAssign,
    BitAnd,
    BitAndAssign,
    BitOr,
    BitOrAssign,
)]
pub struct Byte(u8);

impl Byte {
    pub const fn new(val: u8) -> Self {
        Byte(val)
    }

    pub fn zero() -> Self {
        Self::new(0)
    }

    pub fn get(&self) -> u8 {
        self.0
    }

    pub fn map(self, f: impl FnOnce(u8) -> u8) -> Self {
        Self::new(f(self.0))
    }

    /// Shifts all bits one step to left, prepending the passed in carry bit and wrapping
    /// the truncated bits to the end and returns the new carry bit.
    ///
    /// Here is a small example
    ///
    /// Actual bit: 1010 1100
    /// carry:      true
    /// prepended:  1 1010 1100
    ///             ↑
    ///             The Carry bit (true => 1) is prepended
    /// shifted:    1 0101 1001
    ///             ↑
    ///             This is the output value (new carry) of the method
    /// The resulting byte is: 0101 1001
    pub fn rotate_left_through_carry(&mut self, carry: bool) -> bool {
        let out = (0b1000_0000 & self.0) != 0;
        self.0 = (self.0 << 1) | (carry as u8);
        out
    }

    /// Shifts all bits one step to right, prepending the passed in carry bit and wrapping
    /// the truncated bits to the end and returns the new carry bit.
    ///
    /// For an example see [`Byte::rotate_left_through_carry`].
    pub fn rotate_right_through_carry(&mut self, carry: bool) -> bool {
        let out = (0b0000_0001 & self.0) != 0;
        self.0 = (self.0 >> 1) | ((carry as u8) << 7);
        out
    }

    /// Adds the given [`Byte`] to this [`Byte`] and returns a tuple containing information
    /// about carry and half carry bits: (carry, half_carry)
    pub fn add_with_carries(&mut self, rhs: Byte) -> (bool, bool) {
        let half_carry = (((self.get() & 0x0f) + (rhs.get() & 0x0f)) & 0x10) == 0x10;
        let carry = self.get().checked_add(rhs.get()).is_none();
        *self += rhs;

        (carry, half_carry)
    }

    /// Substracts the given [`Byte`] from this [`Byte`] and returns a tuple containing information
    /// about carry and half carry bits: (carry, half_carry)
    pub fn sub_with_carries(&mut self, rhs: Byte) -> (bool, bool) {
        let half_carry = (self.get() & 0x0f) < (rhs.get() & 0x0f);
        let carry = *self < rhs;
        *self -= rhs;

        (carry, half_carry)
    }

    /// Shifts all bits one step to the left, wrapping the truncated bits to the end and returns
    /// true, if a 1-bit was wrapped around, false otherwise.
    pub fn rotate_left(&mut self) -> bool {
        // Check if a 1-bit is going to be shifted out
        let out = (self.get() & 0b1000_0000) != 0;

        self.0 = self.get().rotate_left(1);

        out
    }

    /// Shifts all bits one step to the right, wrapping the truncated bits to the end and returns
    /// true, if a 1-bit was wrapped around, false otherwise.
    pub fn rotate_right(&mut self) -> bool {
        // Check if a 1-bit is going to be shifted out
        let out = (self.get() & 0b0000_0001) != 0;

        self.0 = self.get().rotate_right(1);

        out
    }

    /// Shifts all bits one step to the left (logical shift), and sets bit 0 to zero and returns
    /// true, if a 1-bit was shifted out, false otherwise.
    pub fn shift_left(&mut self) -> bool {
        // Check if a 1-bit is going to be shifted out
        let out = (self.get() & 0b1000_0000) != 0;

        self.0 = self.get() << 1;

        out
    }

    /// Shifts all bits one step to the right (logical shift), and sets bit 7 to zero and returns
    /// true, if a 1-bit was shifted out, false otherwise.
    pub fn shift_right(&mut self) -> bool {
        // Check if a 1-bit is going to be shifted out
        let out = (self.get() & 0b0000_0001) != 0;

        self.0 = self.get() >> 1;

        out
    }

    /// Shifts all bits one step to the right (arithmetic shift), and preserves the value of
    /// the MSB and returns true, if a 1-bit was shifted out, false otherwise.
    pub fn arithmetic_shift_right(&mut self) -> bool {
        // Check if a 1-bit is going to be shifted out
        let out = (self.get() & 0b0000_0001) != 0;

        self.0 = ((self.get() as i8 ) >> 1) as u8;

        out
    }
}

impl Add for Byte {
    type Output = Self;

    fn add(self, rhs: Self) -> Self {
        Byte(self.0.wrapping_add(rhs.0))
    }
}

impl Add<u8> for Byte {
    type Output = Self;

    fn add(self, rhs: u8) -> Self {
        Byte(self.0.wrapping_add(rhs))
    }
}

impl AddAssign for Byte {
    fn add_assign(&mut self, rhs: Self) {
        *self = *self + rhs;
    }
}

impl AddAssign<u8> for Byte {
    fn add_assign(&mut self, rhs: u8) {
        *self = *self + rhs;
    }
}

impl Sub for Byte {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self {
        Byte(self.0.wrapping_sub(rhs.0))
    }
}

impl Sub<u8> for Byte {
    type Output = Self;

    fn sub(self, rhs: u8) -> Self {
        Byte(self.0.wrapping_sub(rhs))
    }
}

impl SubAssign for Byte {
    fn sub_assign(&mut self, rhs: Self) {
        *self = *self - rhs;
    }
}

impl SubAssign<u8> for Byte {
    fn sub_assign(&mut self, rhs: u8) {
        *self = *self - rhs;
    }
}

impl PartialEq<u8> for Byte {
    fn eq(&self, other: &u8) -> bool {
        self.0 == *other
    }
}

impl Debug for Byte {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "0x{:02x}", self.0)
    }
}

impl Display for Byte {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        Debug::fmt(self, f)
    }
}


/// This represents a value consisting of two [`Byte`]s (e.g. an address).
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Word(u16);


impl Word {
    pub fn new(val: u16) -> Self {
        Word(val)
    }

    pub fn zero() -> Self {
        Self::new(0)
    }

    pub fn get(&self) -> u16 {
        self.0
    }

    /// Creates a word from the two given bytes.
    pub fn from_bytes(lsb: Byte, msb: Byte) -> Self {
        let val = ((msb.get() as u16) << 8) | lsb.get() as u16;

        Self::new(val)
    }

    /// Destructs the word into two [`Byte`]s.
    ///
    /// The first [`Byte`] in the returned tuple is the lsb and the second
    /// [`Byte`] in the returned tuple is the msb (i.e. returned tuple: (lsb,
    /// msb)).
    pub fn into_bytes(self) -> (Byte, Byte) {
        let lsb = (self.0 & 0xff) as u8;
        let msb = ((self.0 >> 8) & 0xff) as u8;

        (Byte::new(lsb), Byte::new(msb))
    }
}

impl Add for Word {
    type Output = Self;

    fn add(self, rhs: Self) -> Self {
        Word(self.0.wrapping_add(rhs.0))
    }
}

impl Add<i8> for Word {
    type Output = Self;

    fn add(self, rhs: i8) -> Self {
        Word((self.0 as i16).wrapping_add(rhs as i16) as u16)
    }
}

impl Add<u8> for Word {
    type Output = Self;

    fn add(self, rhs: u8) -> Self {
        self + rhs as u16
    }
}


impl Add<u16> for Word {
    type Output = Self;

    fn add(self, rhs: u16) -> Self {
        Word(self.0.wrapping_add(rhs as u16))
    }
}

impl Add<Byte> for Word {
    type Output = Self;

    fn add(self, rhs: Byte) -> Self {
        Word(self.0.wrapping_add(rhs.get() as u16))
    }
}

impl AddAssign<i8> for Word {
    fn add_assign(&mut self, rhs: i8) {
        *self = *self + rhs;
    }
}

impl AddAssign<u8> for Word {
    fn add_assign(&mut self, rhs: u8) {
        *self += rhs as u16;
    }
}

impl AddAssign<u16> for Word {
    fn add_assign(&mut self, rhs: u16) {
        *self = *self + rhs;
    }
}

impl AddAssign<Byte> for Word {
    fn add_assign(&mut self, rhs: Byte) {
        *self = *self + rhs;
    }
}

impl Sub for Word {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self {
        Word(self.0.wrapping_sub(rhs.0))
    }
}

impl Sub<u16> for Word {
    type Output = Self;

    fn sub(self, rhs: u16) -> Self {
        Word(self.0.wrapping_sub(rhs as u16))
    }
}

impl SubAssign<u16> for Word {
    fn sub_assign(&mut self, rhs: u16) {
        *self = *self - rhs;
    }
}

impl PartialEq<u16> for Word {
    fn eq(&self, other: &u16) -> bool {
        self.0 == *other
    }
}

impl Debug for Word {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "0x{:04x}", self.0)
    }
}

impl Display for Word {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        Debug::fmt(self, f)
    }
}


/// A chunk of Gameboy memory. Can be indexed by `Word`.
pub struct Memory(Box<[Byte]>);

impl Memory {
    /// Returns a slice of memory with the specified length where all bytes are
    /// set to 0.
    pub fn zeroed(len: Word) -> Self {
        Memory(vec![Byte::zero(); len.get() as usize].into_boxed_slice())
    }

    pub fn from_bytes(bytes: &[u8]) -> Self {
        let copy: Vec<_> = bytes.iter().cloned().map(Byte::new).collect();
        Memory(copy.into_boxed_slice())
    }

    pub fn len(&self) -> Word {
        Word::new(self.0.len() as u16)
    }
}

impl Index<Word> for Memory {
    type Output = Byte;
    fn index(&self, index: Word) -> &Self::Output {
        &(*self.0)[index.0 as usize]
    }
}

impl Index<Range<Word>> for Memory {
    type Output = [Byte];
    fn index(&self, index: Range<Word>) -> &Self::Output {
        &(*self.0)[index.start.0 as usize..index.end.0 as usize]
    }
}

impl IndexMut<Word> for Memory {
    fn index_mut(&mut self, index: Word) -> &mut Self::Output {
        &mut (*self.0)[index.0 as usize]
    }
}


// TODO cpu cycles or machine cycles???
/// Numbers of cycles per frame (including v-blank)
pub const CYCLES_PER_FRAME: u64 = 17556;

/// A simple integer to count how many cycles were already executed by the
/// emulator. This allows to check in what part of the frame we currently are.
#[derive(Debug, Display, Clone, Copy)]
pub struct CycleCounter(u64);

impl CycleCounter {
    pub fn zero() -> Self {
        CycleCounter(0)
    }

    /// Returns true, if the counter is exactly btweeen two frames, false otherwise.
    pub fn is_between_frames(&self) -> bool {
        self.0 % CYCLES_PER_FRAME == 0
    }
}

impl AddAssign<u8> for CycleCounter {
    fn add_assign(&mut self, rhs: u8) {
        self.0 += rhs as u64;
    }
}

#[cfg(test)]
mod test {
    use super::*;


    #[test]
    fn test_rotate_left_through_carry() {
        fn run(val: u8, carry: bool) -> (u8, bool) {
            let mut b = Byte::new(val);
            let carry = b.rotate_left_through_carry(carry);
            (b.get(), carry)
        }

        assert_eq!(run(0b1001_0001, false), (0b0010_0010, true));
        assert_eq!(run(0b1001_0001, true), (0b0010_0011, true));
    }

    #[test]
    fn test_byte_add_with_carries() {
        fn run(lhs: u8, rhs: u8) -> (bool, bool) {
            Byte::new(lhs).add_with_carries(Byte::new(rhs))
        }

        assert_eq!(run(0,   0)  , (false,   false));
        assert_eq!(run(0,   255), (false,   false));
        assert_eq!(run(255, 255), (true,    true));
        assert_eq!(run(255, 0)  , (false,   false));
        assert_eq!(run(255, 1)  , (true,    true));
        assert_eq!(run(127, 1)  , (false,   true));
        assert_eq!(run(128, 128), (true,    false));
    }

    #[test]
    fn test_byte_sub_with_carries() {
        fn run(lhs: u8, rhs: u8) -> (bool, bool) {
            Byte::new(lhs).sub_with_carries(Byte::new(rhs))
        }

        assert_eq!(run(0,   0)  , (false,   false));
        assert_eq!(run(0,   255), (true,    true));
        assert_eq!(run(255, 255), (false,   false));
        assert_eq!(run(255, 0)  , (false,   false));
        assert_eq!(run(255, 1)  , (false,   false));
        assert_eq!(run(127, 1)  , (false,   false));
        assert_eq!(run(128, 1)  , (false,   true));
        assert_eq!(run(128, 128), (false,   false));
        assert_eq!(run(127, 128), (true,    false));
    }
}

/// Position of a pixel on a gameboy screen.
#[derive(Clone, Copy, Debug)]
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
#[derive(Clone, Copy, Debug)]
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

    /// Creates a color from a classic gameboy color (2 bit). The given `c`
    /// value has to be 0, 1, 2 or 3.
    pub fn from_greyscale(c: u8) -> Self {
        assert!(c <= 3);
        let v = (3 - c) << 3;
        Self {
            r: v,
            g: v,
            b: v,
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
