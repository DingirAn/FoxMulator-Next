//! Joypad (P1/JOYP) register and button state.
//!
//! P1 at 0xFF00 is active-low: bit 4 selects direction keys, bit 5 selects
//! action buttons, and bits 0-3 read `0` when the corresponding key is held.
//! A high-to-low transition on any selected line requests the joypad interrupt
//! (IF bit 4). Reference: Pan Docs, "Joypad".

/// The eight Game Boy buttons.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Button {
    Right,
    Left,
    Up,
    Down,
    A,
    B,
    Select,
    Start,
}

impl Button {
    /// Bit index inside its group (0-3).
    fn bit(self) -> u8 {
        match self {
            Button::Right | Button::A => 0,
            Button::Left | Button::B => 1,
            Button::Up | Button::Select => 2,
            Button::Down | Button::Start => 3,
        }
    }

    fn is_direction(self) -> bool {
        matches!(
            self,
            Button::Right | Button::Left | Button::Up | Button::Down
        )
    }
}

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct Joypad {
    /// Low nibble of action buttons (bit set = pressed).
    action: u8,
    /// Low nibble of direction buttons (bit set = pressed).
    direction: u8,
    /// Written P1 bits 4-5 (1 = group not selected).
    select: u8,
    /// Previous state of the selected lines (active-low bits).
    prev_lines: u8,
}

impl Joypad {
    pub fn new() -> Self {
        Self {
            action: 0,
            direction: 0,
            select: 0x30,
            prev_lines: 0x0F,
        }
    }

    /// Press or release a button. Returns `true` when this transition should
    /// request the joypad interrupt.
    pub fn set_button(&mut self, button: Button, pressed: bool) -> bool {
        let mask = 1 << button.bit();
        if button.is_direction() {
            if pressed {
                self.direction |= mask;
            } else {
                self.direction &= !mask;
            }
        } else if pressed {
            self.action |= mask;
        } else {
            self.action &= !mask;
        }
        self.check_irq()
    }

    /// Buttons of a group that are currently held.
    pub fn pressed_mask(&self) -> u8 {
        self.read() & 0x0F
    }

    /// Write P1 (only bits 4-5 select the groups). Returns `true` when the
    /// write causes a high-to-low transition on a selected line.
    pub fn write(&mut self, val: u8) -> bool {
        self.select = val & 0x30;
        self.check_irq()
    }

    /// Current P1 value. Unused high bits read as 1.
    pub fn read(&self) -> u8 {
        0xC0 | self.select | self.lines()
    }

    /// Active-low selected input lines.
    fn lines(&self) -> u8 {
        let mut low = 0x0F;
        if (self.select & 0x10) == 0 {
            low &= !self.direction;
        }
        if (self.select & 0x20) == 0 {
            low &= !self.action;
        }
        low
    }

    fn check_irq(&mut self) -> bool {
        let lines = self.lines();
        let falling = self.prev_lines & !lines;
        self.prev_lines = lines;
        falling != 0
    }

    /// Reset to the power-on state (no buttons held, both groups selected).
    pub fn reset(&mut self) {
        *self = Self::new();
    }
}
