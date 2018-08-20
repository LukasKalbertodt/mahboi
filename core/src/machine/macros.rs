// TODO: Docs from @LukasKalbertodt
macro_rules! set_flags {
    ($reg:expr => $z:tt $n:tt $h:tt $c:tt) => {
        let mut byte = $reg.get();
        set_flags!(@bit $z, 0b1000_0000, byte);
        set_flags!(@bit $n, 0b0100_0000, byte);
        set_flags!(@bit $h, 0b0010_0000, byte);
        set_flags!(@bit $c, 0b0001_0000, byte);
        $reg = $crate::primitives::Byte::new(byte);
    };
    (@bit 0, $mask:expr, $reg:expr) => {
        $reg &= !$mask;
    };
    (@bit 1, $mask:expr, $reg:expr) => {
        $reg |= $mask;
    };
    (@bit -, $mask:expr, $reg:expr) => {};
    (@bit $v:expr, $mask:expr, $reg:expr) => {
        if $v {
            set_flags!(@bit 1, $mask, $reg);
        } else {
            set_flags!(@bit 0, $mask, $reg);
        }
    };
}
