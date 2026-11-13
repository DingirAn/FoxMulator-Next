//! Game Boy timer: DIV, TIMA, TMA and TAC.
//!
//! Reference: Pan Docs, "Timer and Divider Registers".
//! DIV is the upper 8 bits of a 16-bit counter clocked every T-cycle, so the
//! visible DIV register increments every 256 T-cycles (16384 Hz).
//! TIMA increments at the frequency selected by TAC:
//!   00 = 1024 cycles (4096 Hz), 01 = 16 cycles (262144 Hz),
//!   10 = 64 cycles (65536 Hz),  11 = 256 cycles (16384 Hz).

pub const DIV: u16 = 0xFF04;
pub const TIMA: u16 = 0xFF05;
pub const TMA: u16 = 0xFF06;
pub const TAC: u16 = 0xFF07;
pub const TIMER_INTERRUPT_BIT: u8 = 0x04;

#[derive(Default, Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Timer {
    /// Visible DIV register (upper 8 bits of the internal counter).
    pub div: u8,
    pub tima: u8,
    pub tma: u8,
    pub tac: u8,
    /// Internal divider counter. Kept wider than the hardware's 16 bits so
    /// several ticks can be applied at once without losing the visible-DIV
    /// delta. The visible `div` register is derived from the upper 8 bits.
    internal_div: u64,
    /// Phase accumulator for TIMA.
    tima_counter: u32,
    /// Set when TIMA overflows; consumed by the MMU to raise IF bit 2.
    interrupt: bool,
}

impl Timer {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn apu_frame_phase(&self) -> u32 {
        (self.internal_div % 8_192) as u32
    }

    /// Advance the timer by `cycles` T-cycles.
    pub fn tick(&mut self, cycles: u32) -> u32 {
        if cycles == 0 {
            return 0;
        }

        // DIV: visible register increments once per 256 T-cycles.
        let old_internal_div = self.internal_div;
        let old_div = old_internal_div >> 8;
        self.internal_div = self.internal_div.wrapping_add(cycles as u64);
        let new_div = self.internal_div >> 8;
        self.div = self.div.wrapping_add(new_div.wrapping_sub(old_div) as u8);
        let frame_events = (self.internal_div / 8_192) - (old_internal_div / 8_192);

        // TIMA.
        if (self.tac & 0x04) != 0 {
            let period = match self.tac & 0x03 {
                0 => 1024,
                1 => 16,
                2 => 64,
                _ => 256,
            };
            self.tima_counter += cycles;
            while self.tima_counter >= period {
                self.tima_counter -= period;
                self.increment_tima();
            }
        }
        frame_events as u32
    }

    fn increment_tima(&mut self) {
        if self.tima == 0xFF {
            self.tima = self.tma;
            self.interrupt = true;
        } else {
            self.tima = self.tima.wrapping_add(1);
        }
    }

    /// Read one of the timer registers. Unused TAC bits read as 1.
    pub fn read_register(&self, addr: u16) -> u8 {
        match addr {
            DIV => self.div,
            TIMA => self.tima,
            TMA => self.tma,
            TAC => self.tac | 0xF8,
            _ => 0xFF,
        }
    }

    /// Write one of the timer registers.
    pub fn write_register(&mut self, addr: u16, val: u8) -> bool {
        let mut apu_frame_event = false;
        match addr {
            DIV => {
                apu_frame_event = self.apu_frame_phase() >= 4_096;
                // Any write to DIV resets the whole internal divider.
                self.internal_div = 0;
                self.div = 0;
                self.tima_counter = 0;
            }
            TIMA => self.tima = val,
            TMA => self.tma = val,
            TAC => self.tac = val & 0x07,
            _ => {}
        }
        apu_frame_event
    }

    /// Take (and clear) a pending TIMA overflow interrupt.
    pub fn take_interrupt(&mut self) -> bool {
        let pending = self.interrupt;
        self.interrupt = false;
        pending
    }

    pub fn interrupt_pending(&self) -> bool {
        self.interrupt
    }
}
