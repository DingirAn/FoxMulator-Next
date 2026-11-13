//! Flags del SM83: Z (Zero), N (Subtract), H (Half-carry), C (Carry)
/// El registro F almacena los 4 flags en sus bits correctos.

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Flags(u8);

impl Flags {
    /// Z = bit 7, N = bit 6, H = bit 5, C = bit 4
    pub const Z: u8 = 0x80;
    pub const N: u8 = 0x40;
    pub const H: u8 = 0x20;
    pub const C: u8 = 0x10;

    #[inline]
    pub fn new() -> Self {
        Flags(0)
    }

    #[inline]
    pub fn value(&self) -> u8 {
        self.0
    }

    /// Sets all flags from raw byte (only bits 4-7 are used)
    #[inline]
    pub fn set_value(&mut self, val: u8) {
        self.0 = (val & 0xF0) | (self.0 & 0x0F);
    }

    #[inline]
    pub fn z(&self) -> bool {
        (self.0 & Self::Z) != 0
    }
    #[inline]
    pub fn n(&self) -> bool {
        (self.0 & Self::N) != 0
    }
    #[inline]
    pub fn h(&self) -> bool {
        (self.0 & Self::H) != 0
    }
    #[inline]
    pub fn c(&self) -> bool {
        (self.0 & Self::C) != 0
    }

    #[inline]
    pub fn set_z(&mut self, val: bool) {
        self.0 = (self.0 & !Self::Z) | (val as u8 * Self::Z);
    }
    #[inline]
    pub fn set_n(&mut self, val: bool) {
        self.0 = (self.0 & !Self::N) | (val as u8 * Self::N);
    }
    #[inline]
    pub fn set_h(&mut self, val: bool) {
        self.0 = (self.0 & !Self::H) | (val as u8 * Self::H);
    }
    #[inline]
    pub fn set_c(&mut self, val: bool) {
        self.0 = (self.0 & !Self::C) | (val as u8 * Self::C);
    }

    /// Sets flags from result of 8-bit operation
    #[inline]
    pub fn set_from_result_8(val: u8, sub: bool, carry: bool, half_carry: bool) -> u8 {
        let mut f = 0u8;
        if val == 0 {
            f |= Self::Z;
        }
        if sub {
            f |= Self::N;
        }
        if carry {
            f |= Self::C;
        }
        if half_carry {
            f |= Self::H;
        }
        f
    }

    /// Sets flags from result of 16-bit operation (only affects H and C for add/sub)
    #[inline]
    pub fn set_from_result_16(_val: u16, sub: bool, carry: bool, half_carry: bool) -> u8 {
        let mut f = 0u8;
        if carry {
            f |= Self::C;
        }
        if sub {
            f |= Self::N;
        }
        if half_carry {
            f |= Self::H;
        }
        f
    }

    /// Copy from one flag set to another (for PUSH/POP)
    #[inline]
    pub fn copy_from(&mut self, other: &Flags) {
        self.0 = other.0;
    }
}
