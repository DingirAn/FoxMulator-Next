//! Interrupt controller and vector helpers.
//!
//! The Game Boy has five interrupts, each identified by a bit in the IF
//! (0xFF0F) and IE (0xFFFF) registers:
//!
//! | Bit | Interrupt | Vector |
//! |-----|-----------|--------|
//! | 0   | VBlank    | 0x0040 |
//! | 1   | LCD STAT  | 0x0048 |
//! | 2   | Timer     | 0x0050 |
//! | 3   | Serial    | 0x0058 |
//! | 4   | Joypad    | 0x0060 |
//!
//! Reference: Pan Docs, "Interrupts".

/// VBlank interrupt bit (IF/IE bit 0).
pub const VBLANK: u8 = 0x01;
/// LCD STAT interrupt bit (IF/IE bit 1).
pub const LCD_STAT: u8 = 0x02;
/// Timer interrupt bit (IF/IE bit 2).
pub const TIMER: u8 = 0x04;
/// Serial interrupt bit (IF/IE bit 3).
pub const SERIAL: u8 = 0x08;
/// Joypad interrupt bit (IF/IE bit 4).
pub const JOYPAD: u8 = 0x10;

/// All five interrupt bits.
pub const ALL: u8 = VBLANK | LCD_STAT | TIMER | SERIAL | JOYPAD;

/// Interrupt vector for a bit number (0-4), or `None` if out of range.
pub fn vector_for_bit(bit: u8) -> Option<u16> {
    match bit {
        0 => Some(0x0040),
        1 => Some(0x0048),
        2 => Some(0x0050),
        3 => Some(0x0058),
        4 => Some(0x0060),
        _ => None,
    }
}

/// Highest-priority pending interrupt bit (lowest set bit) from `if_reg & ie_reg`.
/// Returns `None` when no interrupt is pending.
pub fn highest_priority(if_reg: u8, ie_reg: u8) -> Option<u8> {
    let pending = if_reg & ie_reg & ALL;
    if pending == 0 {
        None
    } else {
        Some(pending.trailing_zeros() as u8)
    }
}

/// Vector for the highest-priority pending interrupt.
pub fn pending_vector(if_reg: u8, ie_reg: u8) -> Option<u16> {
    highest_priority(if_reg, ie_reg).and_then(vector_for_bit)
}

/// Simple interrupt controller that stores enabled and pending flags.
///
/// `Mmu` owns the canonical IF/IE registers; this type is a convenience for
/// callers that want to model the controller independently.
#[derive(Debug, Clone, Default)]
pub struct InterruptController {
    /// Enabled interrupts (IE).
    pub enabled: u8,
    /// Pending interrupts (IF).
    pub flag: u8,
}

impl InterruptController {
    pub fn new() -> Self {
        Self {
            enabled: 0,
            flag: 0,
        }
    }

    /// Enable an interrupt.
    pub fn enable(&mut self, mask: u8) {
        self.enabled |= mask & ALL;
    }

    /// Disable an interrupt.
    pub fn disable(&mut self, mask: u8) {
        self.enabled &= !mask;
    }

    /// Set a pending interrupt.
    pub fn set_flag(&mut self, mask: u8) {
        self.flag |= mask & ALL;
    }

    /// Clear a pending interrupt.
    pub fn clear_flag(&mut self, mask: u8) {
        self.flag &= !mask;
    }

    /// Check if any interrupt is both enabled and pending.
    pub fn pending(&self) -> bool {
        self.enabled & self.flag & ALL != 0
    }

    /// Highest-priority pending enabled interrupt bit.
    pub fn pending_bit(&self) -> Option<u8> {
        highest_priority(self.flag, self.enabled)
    }

    /// Highest-priority pending interrupt vector.
    pub fn pending_vector(&self) -> Option<u16> {
        pending_vector(self.flag, self.enabled)
    }
}
