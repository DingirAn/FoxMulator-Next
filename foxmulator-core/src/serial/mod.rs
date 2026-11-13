//! Serial port (SB/SC) and link-cable byte transfer.
//!
//! Internal-clock transfers shift one byte every 4096 T-cycles (8192 Hz) and
//! raise the serial interrupt (IF bit 3) on completion. With no link partner
//! the received byte is 0xFF, matching a disconnected Game Boy.
//!
//! Reference: Pan Docs, "Serial Data Transfer (Link Cable)".

use std::collections::VecDeque;

pub const SB_ADDR: u16 = 0xFF01;
pub const SC_ADDR: u16 = 0xFF02;
pub const SERIAL_INTERRUPT_BIT: u8 = 0x08;
/// T-cycles for one 8-bit transfer at the normal 8192 Hz clock.
pub const TRANSFER_CYCLES: u32 = 4096;

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct Serial {
    /// Shift register / data byte.
    sb: u8,
    /// Control register (bit 7 start, bit 0 internal clock).
    sc: u8,
    /// Bytes queued for a link partner to read.
    tx_buffer: VecDeque<u8>,
    /// Bytes waiting to be shifted in from a link partner.
    rx_buffer: VecDeque<u8>,
    /// T-cycles elapsed in the current transfer.
    elapsed: u32,
    transferring: bool,
    /// Byte captured when the transfer started, queued on completion.
    shift_byte: u8,
    /// True when a completed transfer used the idle value and can still be
    /// completed by a partner that finishes a few CPU instructions later.
    awaiting_partner: bool,
}

impl Serial {
    pub fn new() -> Self {
        Self::default()
    }

    /// Queue a byte for transmission without starting a transfer. Kept for the
    /// standalone link API used by tests and the dual-console front end.
    pub fn write(&mut self, byte: u8) {
        self.tx_buffer.push_back(byte);
    }

    /// Drain bytes that have completed transmission (e.g. Blargg test output).
    pub fn take_tx(&mut self) -> Vec<u8> {
        self.tx_buffer.drain(..).collect()
    }

    /// Return number of bytes queued for transmission.
    pub fn tx_len(&self) -> usize {
        self.tx_buffer.len()
    }

    /// Read a byte queued by a link partner.
    pub fn read(&mut self) -> Option<u8> {
        self.rx_buffer.pop_front()
    }

    /// Return number of queued bytes in the receive buffer.
    pub fn rx_len(&self) -> usize {
        self.rx_buffer.len()
    }

    /// Deliver a byte from a link partner.
    pub fn receive(&mut self, byte: u8) {
        if self.awaiting_partner {
            self.sb = byte;
            self.awaiting_partner = false;
        } else {
            self.rx_buffer.push_back(byte);
        }
    }

    /// Start a transfer when SC bit 7 is set. Bit 0 selects who supplies the
    /// clock; external-clock transfers wait for `clock_external`.
    fn maybe_start(&mut self, val: u8) {
        self.sc = val & 0x81;
        if (val & 0x80) != 0 {
            self.transferring = true;
            self.elapsed = 0;
            // Capture the byte now: the test framework may overwrite SB before
            // the 4096-cycle transfer has completed.
            self.shift_byte = self.sb;
        } else if (val & 0x80) == 0 {
            self.transferring = false;
        }
    }

    /// Advance an in-progress internal-clock transfer. Returns `true` when the
    /// transfer completes on this call (so the caller can raise IF bit 3).
    pub fn tick(&mut self, cycles: u32) -> bool {
        if !self.transferring || (self.sc & 0x01) == 0 {
            return false;
        }
        self.elapsed = self.elapsed.saturating_add(cycles);
        if self.elapsed >= TRANSFER_CYCLES {
            // Without a partner the input line idles high.
            let received = self.rx_buffer.pop_front().unwrap_or(0xFF);
            self.complete(received);
            true
        } else {
            false
        }
    }

    fn complete(&mut self, received: u8) {
        self.transferring = false;
        self.elapsed = 0;
        self.sc &= !0x80;
        self.tx_buffer.push_back(self.shift_byte);
        self.sb = received;
        self.awaiting_partner = false;
    }

    /// Supply one complete byte clocked by an external serial partner.
    /// Returns `true` when an external-clock transfer completes.
    pub fn clock_external(&mut self, received: u8) -> bool {
        if !self.transferring || (self.sc & 0x01) != 0 {
            return false;
        }
        self.complete(received);
        true
    }

    /// Read SB or SC. Unused SC bits read as 1 (DMG behaviour).
    pub fn read_register(&self, addr: u16) -> u8 {
        match addr {
            SB_ADDR => self.sb,
            SC_ADDR => (self.sc & 0x81) | 0x7E,
            _ => 0xFF,
        }
    }

    /// Write SB or SC.
    pub fn write_register(&mut self, addr: u16, val: u8) {
        match addr {
            SB_ADDR => self.sb = val,
            SC_ADDR => self.maybe_start(val),
            _ => {}
        }
    }

    /// True while a transfer is in progress.
    pub fn busy(&self) -> bool {
        self.transferring
    }

    /// True when the active transfer is driven by this console's clock.
    pub fn internal_clock_active(&self) -> bool {
        self.transferring && (self.sc & 0x01) != 0
    }

    /// Number of T-cycles until the active internal-clock transfer completes.
    pub fn cycles_remaining(&self) -> u32 {
        if self.transferring && self.sc & 1 != 0 {
            TRANSFER_CYCLES.saturating_sub(self.elapsed)
        } else {
            0
        }
    }

    /// Byte currently being shifted out, if a transfer is active.
    pub fn tx_shift_byte(&self) -> Option<u8> {
        self.transferring.then_some(self.shift_byte)
    }

    /// Replace the shift register directly (used by the dual-console link).
    pub fn set_sb(&mut self, value: u8) {
        self.sb = value;
    }

    pub fn sb(&self) -> u8 {
        self.sb
    }

    pub fn reset(&mut self) {
        *self = Self::new();
    }
}
