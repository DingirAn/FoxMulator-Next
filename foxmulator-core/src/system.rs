//! High-level emulator system: connects the CPU, MMU, PPU and peripherals.
//!
//! `System` owns the CPU, MMU and PPU and advances them in lock-step. Each
//! instruction is executed by the CPU, which returns its T-cycle cost; those
//! cycles are then handed to the MMU (timer, serial, APU) and the PPU.
//!
//! Reference: Pan Docs, "CPU Instruction Set" and the FoxMulator JavaScript
//! main loop in `FoxMulator_1.0.1_Link_Dual.html`.

use crate::bootrom;
use crate::cpu::Cpu;
use crate::joypad::Button;
use crate::mmu::Mmu;
use crate::ppu::Ppu;
use crate::{EmuState, Pixel};

/// Complete emulated Game Boy.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct System {
    pub cpu: Cpu,
    pub mmu: Mmu,
    pub ppu: Ppu,
    pub state: EmuState,
    /// True when the emulated machine is a Game Boy Color.
    pub cgb: bool,
}

impl Default for System {
    fn default() -> Self {
        Self::new(false)
    }
}

impl System {
    const STATE_MAGIC: &'static [u8] = b"FOXSTATE1\n";
    /// Create a DMG (`cgb = false`) or CGB (`cgb = true`) system.
    pub fn new(cgb: bool) -> Self {
        let mut system = Self {
            cpu: Cpu::new(),
            mmu: Mmu::new(),
            ppu: Ppu::new(),
            state: EmuState::Running,
            cgb,
        };
        system.reset();
        system
    }

    /// Whether a cartridge header requests CGB hardware.
    ///
    /// `0x80` denotes a dual-mode cartridge. FoxMulator prefers CGB hardware
    /// for it, matching a Game Boy Color rather than falling back to DMG mode.
    /// `0xC0` requires CGB hardware; `0x00` and unknown values use DMG mode.
    pub fn cgb_mode_for_rom(data: &[u8]) -> bool {
        matches!(data.get(0x0143), Some(0x80 | 0xC0))
    }

    /// Create and initialize the appropriate hardware model before loading a
    /// ROM. This keeps CPU post-boot registers, CGB memory banks, palettes and
    /// peripherals consistent with the cartridge header from the first cycle.
    pub fn from_rom(data: Vec<u8>) -> Self {
        let mut system = Self::new(Self::cgb_mode_for_rom(&data));
        system.load_cartridge(data);
        system
    }

    /// Create a system and start it through FoxMulator's embedded SameBoy boot ROM.
    pub fn from_rom_with_embedded_boot(data: Vec<u8>) -> Result<Self, String> {
        let cgb = Self::cgb_mode_for_rom(&data);
        Self::from_model_with_embedded_boot(data, cgb)
    }

    /// Create a system with the integrated boot ROM for an explicitly selected model.
    pub fn from_model_with_embedded_boot(data: Vec<u8>, cgb: bool) -> Result<Self, String> {
        let mut system = Self::new(cgb);
        system.load_cartridge(data);
        system
            .mmu
            .load_integrated_boot_rom(bootrom::for_model(cgb))?;
        system.cpu.start_boot_rom();
        system.mmu.cgb = cgb;
        system.mmu.write_byte(crate::mmu::LCDC_ADDR, 0x00);
        system.ppu.reset(&mut system.mmu);
        Ok(system)
    }

    /// Load a cartridge ROM image.
    pub fn load_cartridge(&mut self, data: Vec<u8>) {
        self.mmu.load_cartridge(data);
    }

    /// Reset CPU and PPU to the post-boot state.
    pub fn reset(&mut self) {
        self.cpu.post_boot(self.cgb);
        self.mmu.cgb = self.cgb;
        self.mmu.vram_bank = 0;
        self.mmu.wram_bank = 1;
        self.mmu.dma_source = 0;
        self.mmu.dma_active = false;
        self.mmu.hdma_source = 0;
        self.mmu.hdma_destination = 0x8000;
        self.mmu.hdma_remaining = 0;
        self.mmu.hdma_active = false;
        self.mmu.hdma_hblank_mode = false;
        self.mmu.apu.set_cgb(self.cgb);
        // A cartridge starts after the boot ROM with the LCD enabled. This is
        // required by CGB programs such as Pokémon Crystal, which wait for a
        // visible LY value before continuing initialization.
        self.mmu.write_byte(crate::mmu::LCDC_ADDR, 0x91);
        self.mmu.double_speed = false;
        self.mmu.key1_armed = false;
        self.mmu.opri = 0;
        self.mmu.pre_write_tick = 0;
        self.ppu.reset(&mut self.mmu);
        self.state = EmuState::Running;
    }

    /// Execute exactly one CPU instruction and advance peripherals by the
    /// number of T-cycles it consumed. Returns the cycles spent.
    pub fn step_instruction(&mut self) -> u32 {
        if self.state != EmuState::Running {
            return 0;
        }
        // The CPU advances the MMU clock itself so that data-memory accesses
        // land on the correct M-cycle. Recover how much peripheral time that
        // consumed and give the PPU exactly the same amount, preserving
        // double-speed scaling.
        let peripheral_before = self.mmu.peripheral_ticks;
        let cycles = self.cpu.step(&mut self.mmu);
        let peripheral_cycles = (self.mmu.peripheral_ticks - peripheral_before) as u32;
        self.ppu.tick(peripheral_cycles, &mut self.mmu);
        cycles
    }

    /// Advance peripherals without executing CPU code. This is used by the
    /// virtual link cable to advance two independent consoles in lock-step.
    pub fn tick_peripherals(&mut self, cycles: u32) {
        let peripheral_before = self.mmu.peripheral_ticks;
        self.mmu.tick(cycles);
        let peripheral_cycles = (self.mmu.peripheral_ticks - peripheral_before) as u32;
        self.ppu.tick(peripheral_cycles, &mut self.mmu);
    }

    /// Run instructions until the PPU completes a frame (or `max_cycles` is
    /// reached). Returns the number of T-cycles executed.
    pub fn step_frame(&mut self, max_cycles: u32) -> u32 {
        let mut total = 0;
        loop {
            if total >= max_cycles {
                break;
            }
            total += self.step_instruction();
            if self.ppu.take_frame_ready() {
                break;
            }
        }
        total
    }

    /// Framebuffer of the last rendered frame.
    pub fn framebuffer(&self) -> &[Pixel] {
        &self.ppu.buffer
    }

    /// Press or release a button.
    pub fn set_button(&mut self, button: Button, pressed: bool) {
        self.mmu.set_button(button, pressed);
    }

    /// Toggle execution without discarding CPU, memory or peripheral state.
    pub fn toggle_pause(&mut self) -> bool {
        self.state = match self.state {
            EmuState::Running => EmuState::Paused,
            EmuState::Paused => EmuState::Running,
            EmuState::Stopped => EmuState::Stopped,
        };
        self.state == EmuState::Paused
    }

    /// Drain queued stereo audio samples.
    pub fn take_audio_samples(&mut self) -> Vec<f32> {
        self.mmu.apu.take_samples()
    }

    /// Apply one byte clocked by an external serial partner.
    pub fn clock_external_serial(&mut self, received: u8) -> bool {
        self.mmu.clock_external_serial(received)
    }

    /// Save the complete emulator state, including cartridge, CPU, PPU,
    /// memory-mapped peripherals, RTC and serial buffers.
    pub fn save_state(&self, path: &std::path::Path) -> std::io::Result<()> {
        use std::io::{Error, ErrorKind, Write};
        let payload = serde_json::to_vec(self)
            .map_err(|error| Error::new(ErrorKind::InvalidData, error.to_string()))?;
        let temp = path.with_extension("state.tmp");
        let mut file = std::fs::File::create(&temp)?;
        file.write_all(Self::STATE_MAGIC)?;
        file.write_all(&payload)?;
        file.sync_all()?;
        std::fs::rename(temp, path)
    }

    /// CRC32 of the immutable cartridge ROM used to reject cross-ROM states.
    pub fn rom_checksum(&self) -> u32 {
        let Some(cartridge) = self.mmu.cartridge.as_ref() else {
            return 0;
        };
        let mut crc = 0xffff_ffffu32;
        for byte in &cartridge.rom {
            crc ^= u32::from(*byte);
            for _ in 0..8 {
                crc = if crc & 1 != 0 {
                    (crc >> 1) ^ 0xedb8_8320
                } else {
                    crc >> 1
                };
            }
        }
        !crc
    }

    /// Restore a complete state written by [`System::save_state`].
    pub fn load_state(&mut self, path: &std::path::Path) -> std::io::Result<()> {
        use std::io::{Error, ErrorKind};
        let data = std::fs::read(path)?;
        let Some(payload) = data.strip_prefix(Self::STATE_MAGIC) else {
            return Err(Error::new(
                ErrorKind::InvalidData,
                "invalid FoxMulator state header",
            ));
        };
        let restored: Self = serde_json::from_slice(payload)
            .map_err(|error| Error::new(ErrorKind::InvalidData, error.to_string()))?;
        *self = restored;
        Ok(())
    }
}

/// A deterministic virtual cable connecting two independent Game Boy systems.
///
/// Each side retains its own CPU, MMU, serial registers and interrupts. When
/// both internal-clock transfers reach completion in the same tick, their
/// shift bytes are placed on the opposite input line before either serial port
/// completes, reproducing a simultaneous eight-bit exchange.
#[derive(Debug, Default, Clone, Copy)]
pub struct LinkCable;

impl LinkCable {
    pub fn new() -> Self {
        Self
    }

    pub fn tick(&mut self, left: &mut System, right: &mut System, cycles: u32) {
        let left_ready =
            left.mmu.serial.internal_clock_active() && left.mmu.serial.cycles_remaining() <= cycles;
        let right_ready = right.mmu.serial.internal_clock_active()
            && right.mmu.serial.cycles_remaining() <= cycles;
        if left_ready && right_ready {
            if let (Some(left_byte), Some(right_byte)) = (
                left.mmu.serial.tx_shift_byte(),
                right.mmu.serial.tx_shift_byte(),
            ) {
                left.mmu.serial.receive(right_byte);
                right.mmu.serial.receive(left_byte);
            }
        } else if left_ready && right.mmu.serial.busy() {
            // The left console supplies the clock while the right console is
            // in external-clock mode. Feed the right shift byte into the left
            // before advancing it, so both ports complete the same exchange.
            if let (Some(left_byte), Some(right_byte)) = (
                left.mmu.serial.tx_shift_byte(),
                right.mmu.serial.tx_shift_byte(),
            ) {
                left.mmu.serial.receive(right_byte);
                right.mmu.clock_external_serial(left_byte);
            }
        } else if right_ready && left.mmu.serial.busy() {
            // Symmetric case: the right console supplies the clock.
            if let (Some(left_byte), Some(right_byte)) = (
                left.mmu.serial.tx_shift_byte(),
                right.mmu.serial.tx_shift_byte(),
            ) {
                right.mmu.serial.receive(left_byte);
                left.mmu.clock_external_serial(right_byte);
            }
        }
        left.tick_peripherals(cycles);
        right.tick_peripherals(cycles);
    }

    /// Deliver completed serial bytes between consoles. This is intentionally
    /// separate from the clocks so callers may run each CPU independently.
    pub fn pump(&mut self, left: &mut System, right: &mut System) {
        for byte in left.mmu.serial.take_tx() {
            right.mmu.serial.receive(byte);
        }
        for byte in right.mmu.serial.take_tx() {
            left.mmu.serial.receive(byte);
        }
    }

    /// Execute one instruction on each console and pump the cable between
    /// them. This is the convenient scheduler for a dual-console frontend.
    pub fn step(&mut self, left: &mut System, right: &mut System) {
        left.step_instruction();
        self.pump(left, right);
        right.step_instruction();
        self.pump(left, right);
    }
}
