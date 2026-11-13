//! Memory Management Unit.
//!
//! Implements the Game Boy / Game Boy Color 16-bit address map:
//!
//! | Range           | Contents                                  |
//! |-----------------|-------------------------------------------|
//! | 0000-3FFF       | Cartridge ROM bank 0 (or MBC register)     |
//! | 4000-7FFF       | Cartridge ROM switchable bank              |
//! | 8000-9FFF       | VRAM                                       |
//! | A000-BFFF       | Cartridge RAM (if enabled)                 |
//! | C000-DFFF       | WRAM                                       |
//! | E000-FDFF       | Echo RAM (mirror of C000-DDFF)             |
//! | FE00-FE9F       | OAM                                        |
//! | FEA0-FEFF       | Unusable (reads 0xFF)                      |
//! | FF00-FF7F       | I/O registers                              |
//! | FF80-FFFE       | HRAM                                       |
//! | FFFF            | IE                                         |
//!
//! Reference: Pan Docs, "Memory Map".

use crate::apu::Apu;
use crate::cartridge::{Cartridge, MbcType};
use crate::joypad::Joypad;
use crate::serial::Serial;
use crate::timer_mod::Timer;

pub const IF_ADDR: u16 = 0xFF0F;
pub const IE_ADDR: u16 = 0xFFFF;
pub const JOYP_ADDR: u16 = 0xFF00;
pub const DIV_ADDR: u16 = 0xFF04;
pub const TIMA_ADDR: u16 = 0xFF05;
pub const TMA_ADDR: u16 = 0xFF06;
pub const TAC_ADDR: u16 = 0xFF07;
pub const LCDC_ADDR: u16 = 0xFF40;
pub const STAT_ADDR: u16 = 0xFF41;
pub const SCY_ADDR: u16 = 0xFF42;
pub const SCX_ADDR: u16 = 0xFF43;
pub const LY_ADDR: u16 = 0xFF44;
pub const LYC_ADDR: u16 = 0xFF45;
pub const DMA_ADDR: u16 = 0xFF46;
pub const BGP_ADDR: u16 = 0xFF47;
pub const OBP0_ADDR: u16 = 0xFF48;
pub const OBP1_ADDR: u16 = 0xFF49;
pub const WY_ADDR: u16 = 0xFF4A;
pub const WX_ADDR: u16 = 0xFF4B;
pub const VBK_ADDR: u16 = 0xFF4F;
pub const BCPS_ADDR: u16 = 0xFF68;
pub const BCPD_ADDR: u16 = 0xFF69;
pub const OCPS_ADDR: u16 = 0xFF6A;
pub const OCPD_ADDR: u16 = 0xFF6B;
pub const OPRI_ADDR: u16 = 0xFF6C;
pub const SVBK_ADDR: u16 = 0xFF70;
pub const HDMA1_ADDR: u16 = 0xFF51;
pub const HDMA2_ADDR: u16 = 0xFF52;
pub const HDMA3_ADDR: u16 = 0xFF53;
pub const HDMA4_ADDR: u16 = 0xFF54;
pub const HDMA5_ADDR: u16 = 0xFF55;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Mmu {
    /// Backing storage for the full 64 KiB address space. Cartridge ROM and
    /// cartridge RAM are routed separately when a cartridge is present.
    memory: Vec<u8>,
    /// CGB VRAM bank 1, selected through VBK. Bank 0 remains in `memory`.
    vram_bank_one: Vec<u8>,
    /// CGB switchable WRAM banks 1-7. Bank 0 remains in `memory`.
    wram_banks: Vec<u8>,
    pub cartridge: Option<Cartridge>,
    /// Integrated DMG/CGB boot ROM mapped until FF50 is written.
    #[serde(skip)]
    boot_rom: Option<&'static [u8]>,
    pub boot_rom_enabled: bool,

    // MBC state.
    pub ram_enabled: bool,
    pub current_rom_bank: u16,
    /// 0 = ROM banking mode, 1 = RAM banking mode (MBC1).
    pub banking_mode: u8,
    pub ram_bank: u8,
    pub external_ram: Vec<u8>,
    /// MBC3 RTC latch / register select state (time is not advanced here yet).
    pub rtc_latch: u8,
    pub rtc_registers: [u8; 5],
    rtc_latched: [u8; 5],
    rtc_last_unix: u64,

    /// DIV/TIMA/TMA/TAC registers and their internal counters.
    pub timer: Timer,
    /// Audio channels, frame sequencer and sample queue.
    pub apu: Apu,
    /// Button state and P1/JOYP register.
    pub joypad: Joypad,
    /// Serial port and link-cable buffers.
    pub serial: Serial,
    /// True when emulating a Game Boy Color. Controls CGB-only registers.
    pub cgb: bool,
    /// CGB double-speed state reported through KEY1 (0xFF4D).
    pub double_speed: bool,
    /// Set when software writes bit 0 of KEY1 to arm a speed switch.
    pub key1_armed: bool,
    /// CGB VRAM bank selector (0 or 1).
    pub vram_bank: u8,
    /// CGB WRAM bank selector (1-7; zero writes select bank 1).
    pub wram_bank: u8,
    /// CGB object priority mode: 0 selects OAM order, 1 selects X order.
    pub opri: u8,
    /// OAM DMA source page currently latched by FF46.
    pub dma_source: u8,
    /// True while the 160-byte OAM DMA transfer is in progress.
    pub dma_active: bool,
    /// Number of bytes already transferred by the current OAM DMA.
    dma_index: u16,
    /// Peripheral T-cycles accumulated toward the next OAM DMA byte.
    dma_phase: u8,
    /// CGB VRAM DMA source/destination and remaining 16-byte blocks.
    pub hdma_source: u16,
    pub hdma_destination: u16,
    pub hdma_remaining: u8,
    pub hdma_active: bool,
    pub hdma_hblank_mode: bool,
    /// CGB background and object palette RAM (64 bytes each).
    pub cgb_bg_palette: Vec<u8>,
    pub cgb_obj_palette: Vec<u8>,
    cgb_bg_palette_index: u8,
    cgb_obj_palette_index: u8,
    /// Leftover CPU cycles when dividing peripheral time in double speed.
    speed_accum: u32,
    /// APU frame-sequencer phase in peripheral T-cycles. Unlike TIMA, this
    /// remains on the normal system clock in CGB double-speed mode.
    apu_frame_phase: u32,
    /// Total peripheral T-cycles elapsed. `System` uses this to keep the PPU
    /// in lock-step with the MMU-owned clock.
    pub peripheral_ticks: u64,
    /// T-cycles to apply immediately before the next data write. The CPU uses
    /// this to place the write of a read-modify-write instruction on its last
    /// M-cycle (Pan Docs, instruction timing).
    pub pre_write_tick: u32,
    /// T-cycles elapsed in the current PPU scanline. `tick` advances this value
    /// and the PPU consumes/resets it, so the CPU can place DMG OAM-bug
    /// accesses on the OAM row the PPU is scanning.
    pub ppu_line_cycles: u32,
    /// Current PPU mode, mirrored here for the DMG OAM-bug access checks.
    pub ppu_mode: u8,
    /// True while the LCD is enabled (LCDC bit 7), used to detect the LCD
    /// off-to-on transition that shortens the first scanline.
    ppu_lcd_on: bool,
    /// True during the first scanline after the LCD is enabled.
    pub ppu_first_line: bool,
    /// OAM-search state mirrored at M-cycle granularity for the DMG OAM bug.
    oam_search_row: usize,
    oam_search_phase: u32,
    /// Opt-in, bounded-by-caller write trace for ROM-control and cartridge-RAM
    /// diagnostics. It is disabled in normal emulation and save states.
    #[serde(skip)]
    debug_trace_cart_writes: bool,
    #[serde(skip)]
    debug_cart_writes: Vec<(u16, u8)>,
    #[serde(skip)]
    debug_boot_mmio_events: usize,
    #[serde(skip)]
    debug_vram_events: usize,
}

impl Default for Mmu {
    fn default() -> Self {
        Self {
            memory: vec![0; 0x10000],
            vram_bank_one: vec![0; 0x2000],
            wram_banks: vec![0; 0x7000],
            cartridge: None,
            boot_rom: None,
            boot_rom_enabled: false,
            ram_enabled: false,
            current_rom_bank: 1,
            banking_mode: 0,
            ram_bank: 0,
            external_ram: vec![0; 0x2000],
            rtc_latch: 0,
            rtc_registers: [0; 5],
            rtc_latched: [0; 5],
            rtc_last_unix: 0,
            timer: Timer::new(),
            apu: Apu::new(),
            joypad: Joypad::new(),
            serial: Serial::new(),
            cgb: false,
            double_speed: false,
            key1_armed: false,
            vram_bank: 0,
            wram_bank: 1,
            opri: 0,
            dma_source: 0,
            dma_active: false,
            dma_index: 0,
            dma_phase: 0,
            hdma_source: 0,
            hdma_destination: 0x8000,
            hdma_remaining: 0,
            hdma_active: false,
            hdma_hblank_mode: false,
            cgb_bg_palette: vec![0; 64],
            cgb_obj_palette: vec![0; 64],
            cgb_bg_palette_index: 0,
            cgb_obj_palette_index: 0,
            speed_accum: 0,
            apu_frame_phase: 0,
            peripheral_ticks: 0,
            pre_write_tick: 0,
            ppu_line_cycles: 0,
            ppu_mode: 2,
            ppu_lcd_on: false,
            ppu_first_line: false,
            oam_search_row: 0,
            oam_search_phase: 0,
            debug_trace_cart_writes: false,
            debug_cart_writes: Vec::new(),
            debug_boot_mmio_events: 0,
            debug_vram_events: 0,
        }
    }
}

impl Mmu {
    /// Enable a diagnostic trace of MBC-control and external-RAM writes.
    /// Normal emulation never records these events.
    pub fn set_debug_trace_cart_writes(&mut self, enabled: bool) {
        self.debug_trace_cart_writes = enabled;
        self.debug_cart_writes.clear();
    }

    /// Drain diagnostic cartridge write events in execution order.
    pub fn take_debug_cart_writes(&mut self) -> Vec<(u16, u8)> {
        std::mem::take(&mut self.debug_cart_writes)
    }

    fn trace_cart_write(&mut self, addr: u16, val: u8) {
        // Cartridge SRAM is also a valid stack location for some Blargg
        // multi-ROM harnesses. Keep the diagnostic stream focused on MBC
        // controls and the protocol window instead of every PUSH/CALL.
        if self.debug_trace_cart_writes && (addr < 0x8000 || addr < 0xA100) {
            self.debug_cart_writes.push((addr, val));
        }
    }
    /// Create a new MMU with 64 KiB of zeroed memory.
    pub fn new() -> Self {
        Self::default()
    }

    fn mbc(&self) -> MbcType {
        self.cartridge
            .as_ref()
            .map(|c| c.mbc)
            .unwrap_or(MbcType::RomOnly)
    }

    fn has_rtc_chip(&self) -> bool {
        self.cartridge
            .as_ref()
            .is_some_and(|cart| matches!(cart.rom.get(0x0147), Some(0x0F | 0x10)))
    }

    fn unix_now() -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |duration| duration.as_secs())
    }

    fn advance_rtc_seconds_internal(&mut self, seconds: u64) {
        if seconds == 0 || !self.has_rtc_chip() || self.rtc_registers[4] & 0x40 != 0 {
            return;
        }
        let current = u64::from(self.rtc_registers[0])
            + u64::from(self.rtc_registers[1]) * 60
            + u64::from(self.rtc_registers[2]) * 3_600
            + u64::from(self.rtc_registers[3]) * 86_400
            + u64::from(self.rtc_registers[4] & 1) * 256 * 86_400;
        let total = current + seconds;
        let days = total / 86_400;
        let carry = self.rtc_registers[4] & 0x80;
        if days >= 512 {
            self.rtc_registers[4] |= 0x80;
        }
        let wrapped = total % (512 * 86_400);
        self.rtc_registers[0] = (wrapped % 60) as u8;
        self.rtc_registers[1] = ((wrapped / 60) % 60) as u8;
        self.rtc_registers[2] = ((wrapped / 3_600) % 24) as u8;
        let wrapped_days = wrapped / 86_400;
        self.rtc_registers[3] = wrapped_days as u8;
        self.rtc_registers[4] = (self.rtc_registers[4] & 0x40)
            | carry
            | if days >= 512 { 0x80 } else { 0 }
            | ((wrapped_days >> 8) as u8 & 1);
    }

    fn update_rtc_wall_clock(&mut self) {
        if !self.has_rtc_chip() {
            return;
        }
        let now = Self::unix_now();
        if self.rtc_last_unix == 0 {
            self.rtc_last_unix = now;
            return;
        }
        let elapsed = now.saturating_sub(self.rtc_last_unix);
        self.advance_rtc_seconds_internal(elapsed);
        self.rtc_last_unix = now;
    }

    /// Advance an MBC3 RTC by an exact number of seconds. This is also useful
    /// for deterministic tests and headless emulation; normal execution uses
    /// the host wall clock through [`Mmu::tick`].
    pub fn advance_rtc_seconds(&mut self, seconds: u64) {
        self.update_rtc_wall_clock();
        self.advance_rtc_seconds_internal(seconds);
        self.rtc_last_unix = Self::unix_now();
    }

    /// Return the live RTC registers after applying elapsed host time.
    pub fn rtc_state(&mut self) -> [u8; 5] {
        self.update_rtc_wall_clock();
        self.rtc_registers
    }

    fn rom_bank_count(&self) -> usize {
        self.cartridge
            .as_ref()
            .map(|c| (c.rom.len() / 0x4000).max(1))
            .unwrap_or(1)
    }

    fn rom_byte(&self, addr: u16) -> u8 {
        if self.boot_rom_enabled {
            if let Some(boot) = self.boot_rom.as_ref() {
                let offset = if addr < 0x0100 {
                    Some(addr as usize)
                } else if self.cgb && (0x0200..0x0900).contains(&addr) {
                    // The 0x900-byte CGB image is addressed at its physical
                    // offsets; 0x100..0x1FF is the cartridge-visible gap.
                    Some(addr as usize)
                } else {
                    None
                };
                if let Some(offset) = offset {
                    if let Some(byte) = boot.get(offset) {
                        return *byte;
                    }
                }
            }
        }
        let Some(cart) = self.cartridge.as_ref() else {
            return self.memory[addr as usize];
        };
        let bank = if addr < 0x4000 {
            // In MBC1 RAM-banking mode, the two upper bank bits move the
            // fixed window as well (banks 0x20, 0x40 or 0x60). In ROM mode
            // the fixed window remains bank 0.
            if self.mbc() == MbcType::Mbc1 && self.banking_mode == 1 {
                (self.ram_bank as usize & 0x03) << 5
            } else {
                0
            }
        } else {
            // In RAM-banking mode only the low five bits select the
            // switchable window; the upper bits select the fixed window.
            if self.mbc() == MbcType::Mbc1 && self.banking_mode == 1 {
                self.current_rom_bank as usize & 0x1F
            } else {
                self.current_rom_bank as usize
            }
        };
        let offset = bank * 0x4000 + (addr as usize & 0x3FFF);
        cart.rom.get(offset).copied().unwrap_or(0xFF)
    }

    fn vram_byte(&self, addr: u16) -> u8 {
        let offset = (addr - 0x8000) as usize;
        if self.cgb && self.vram_bank == 1 {
            self.vram_bank_one[offset]
        } else {
            self.memory[addr as usize]
        }
    }

    /// Read VRAM independently of the CPU-selected VBK bank.
    pub fn read_vram_bank(&self, bank: u8, addr: u16) -> u8 {
        let offset = (addr - 0x8000) as usize;
        if self.cgb && bank & 1 != 0 {
            self.vram_bank_one[offset]
        } else {
            self.memory[addr as usize]
        }
    }

    /// Return one CGB palette entry as little-endian RGB555.
    pub fn cgb_palette_word(&self, object: bool, palette: u8, color: u8) -> u16 {
        let ram = if object {
            &self.cgb_obj_palette
        } else {
            &self.cgb_bg_palette
        };
        let offset = (palette & 7) as usize * 8 + (color & 3) as usize * 2;
        u16::from_le_bytes([ram[offset], ram[offset + 1]])
    }

    fn wram_byte(&self, addr: u16) -> u8 {
        if addr < 0xD000 || !self.cgb || self.wram_bank == 0 {
            return self.memory[addr as usize];
        }
        let offset = (self.wram_bank as usize - 1) * 0x1000 + (addr as usize - 0xD000);
        self.wram_banks[offset]
    }

    fn write_vram_byte(&mut self, addr: u16, val: u8) {
        let offset = (addr - 0x8000) as usize;
        if self.cgb && self.vram_bank == 1 {
            self.vram_bank_one[offset] = val;
        } else {
            self.memory[addr as usize] = val;
        }
    }

    fn write_wram_byte(&mut self, addr: u16, val: u8) {
        if addr < 0xD000 || !self.cgb || self.wram_bank == 0 {
            self.memory[addr as usize] = val;
            return;
        }
        let offset = (self.wram_bank as usize - 1) * 0x1000 + (addr as usize - 0xD000);
        self.wram_banks[offset] = val;
    }

    fn dma_read_byte(&self, addr: u16) -> u8 {
        self.read_byte(addr)
    }

    fn copy_oam_dma_byte(&mut self) {
        let source = (u16::from(self.dma_source) << 8) | self.dma_index;
        let value = self.dma_read_byte(source);
        self.memory[0xFE00 + self.dma_index as usize] = value;
        self.dma_index += 1;
        if self.dma_index >= 0xA0 {
            self.dma_active = false;
            self.dma_phase = 0;
        }
    }

    fn copy_hdma_block(&mut self) {
        let source = self.hdma_source;
        let destination = self.hdma_destination;
        if std::env::var_os("FOXMOULATOR_HDMA_TRACE").is_some() {
            eprintln!(
                "HDMA_TRACE source={source:04X} destination={destination:04X} vbk={} remaining={} hblank={}",
                self.vram_bank,
                self.hdma_remaining,
                self.hdma_hblank_mode
            );
        }
        for offset in 0..0x10u16 {
            let value = self.dma_read_byte(source.wrapping_add(offset));
            self.write_vram_byte(
                0x8000 | ((destination.wrapping_add(offset)) & 0x1FFF),
                value,
            );
        }
        self.hdma_source = self.hdma_source.wrapping_add(0x10);
        self.hdma_destination = 0x8000 | ((self.hdma_destination + 0x10) & 0x1FFF);
        self.hdma_remaining = self.hdma_remaining.saturating_sub(1);
        if self.hdma_remaining == 0 {
            self.hdma_active = false;
            self.hdma_hblank_mode = false;
        }
    }

    /// Perform one pending CGB HBlank DMA block. The PPU calls this when it
    /// enters mode 0; a block occupies eight M-cycles (32 T-cycles).
    pub fn hdma_hblank_transfer(&mut self) {
        if self.cgb && self.hdma_active && self.hdma_hblank_mode {
            self.copy_hdma_block();
            self.tick(32);
        }
    }

    /// Read a byte from the given 16-bit address.
    pub fn read_byte(&self, addr: u16) -> u8 {
        match addr {
            0x0000..=0x7FFF => self.rom_byte(addr),
            // Timer registers are backed by the internal Timer, not by the
            // generic I/O backing store.
            DIV_ADDR..=TAC_ADDR => self.timer.read_register(addr),
            // Audio registers and wave RAM are owned by the APU.
            0xFF10..=0xFF3F => self.apu.read_register(addr),
            // Joypad register is generated from the current button state.
            JOYP_ADDR => self.joypad.read(),
            // Serial data (SB) and control (SC).
            0xFF01..=0xFF02 => self.serial.read_register(addr),
            // Interrupt flag: bits 5-7 are unused and always read as 1.
            // Reference: Pan Docs, "Interrupts".
            IF_ADDR => self.memory[IF_ADDR as usize] | 0xE0,
            // KEY1 is a CGB-only register; on DMG the address is unmapped and
            // reads 0xFF. This matters for software that probes the CGB bit.
            0xFF4D => {
                if self.cgb {
                    // KEY1: bit 7 is the current speed, bits 6-1 read as one,
                    // and bit 0 exposes the armed speed-switch request.
                    (if self.double_speed { 0x80 } else { 0 }) | 0x7E | u8::from(self.key1_armed)
                } else {
                    0xFF
                }
            }
            VBK_ADDR => {
                if self.cgb {
                    self.vram_bank & 1
                } else {
                    0xFF
                }
            }
            SVBK_ADDR => {
                if self.cgb {
                    0xF8 | (self.wram_bank & 7)
                } else {
                    0xFF
                }
            }
            HDMA1_ADDR => {
                if self.cgb {
                    (self.hdma_source >> 8) as u8
                } else {
                    0xFF
                }
            }
            HDMA2_ADDR => {
                if self.cgb {
                    (self.hdma_source >> 4) as u8 & 0xF0
                } else {
                    0xFF
                }
            }
            HDMA3_ADDR => {
                if self.cgb {
                    ((self.hdma_destination >> 8) as u8 & 0x1F) | 0x80
                } else {
                    0xFF
                }
            }
            HDMA4_ADDR => {
                if self.cgb {
                    (self.hdma_destination >> 4) as u8 & 0xF0
                } else {
                    0xFF
                }
            }
            HDMA5_ADDR => {
                if !self.cgb || (!self.hdma_active && self.hdma_remaining == 0) {
                    0xFF
                } else {
                    (self.hdma_remaining - 1) & 0x7F | if self.hdma_hblank_mode { 0x80 } else { 0 }
                }
            }
            BCPS_ADDR | OCPS_ADDR => {
                if !self.cgb {
                    0xFF
                } else if addr == BCPS_ADDR {
                    self.cgb_bg_palette_index
                } else {
                    self.cgb_obj_palette_index
                }
            }
            BCPD_ADDR | OCPD_ADDR => {
                if !self.cgb {
                    0xFF
                } else if addr == BCPD_ADDR {
                    self.cgb_bg_palette[(self.cgb_bg_palette_index & 0x3F) as usize]
                } else {
                    self.cgb_obj_palette[(self.cgb_obj_palette_index & 0x3F) as usize]
                }
            }
            OPRI_ADDR => {
                if self.cgb {
                    self.opri & 1
                } else {
                    0xFF
                }
            }
            0x8000..=0x9FFF => self.vram_byte(addr),
            0xC000..=0xDFFF => self.wram_byte(addr),
            0xFE00..=0xFE9F | 0xFF00..=0xFFFE => self.memory[addr as usize],
            0xA000..=0xBFFF => {
                if self.cartridge.is_none() {
                    return self.memory[addr as usize];
                }
                if !self.ram_enabled {
                    return 0xFF;
                }
                let index = self.cart_ram_index(addr);
                match self.mbc() {
                    // MBC2 has 512 × 4 bits and returns 0xF in the high nibble.
                    MbcType::Mbc2 => {
                        0xF0 | (self.external_ram.get(index).copied().unwrap_or(0) & 0x0F)
                    }
                    // MBC3 RTC registers are selected through ram_bank 0x08..=0x0C.
                    MbcType::Mbc3 if self.ram_bank >= 0x08 => {
                        self.rtc_latched[(self.ram_bank - 0x08) as usize]
                    }
                    _ => self.external_ram.get(index).copied().unwrap_or(0xFF),
                }
            }
            // Echo RAM mirrors C000-DDFF.
            0xE000..=0xFDFF => self.wram_byte(addr - 0x2000),
            // Unusable region.
            0xFEA0..=0xFEFF => 0xFF,
            // IE.
            0xFFFF => self.memory[IE_ADDR as usize],
        }
    }

    /// Read through the CPU bus, including LCD mode access restrictions.
    ///
    /// The unrestricted `read_byte` API is intentionally retained for PPU,
    /// DMA and test-bench accesses. During pixel transfer the CPU sees open
    /// bus (`FF`) for VRAM, and during OAM search or pixel transfer it sees
    /// `FF` for OAM. This distinction is required because the PPU must still
    /// be able to fetch the same memory while it owns those buses.
    pub fn cpu_read_byte(&self, addr: u16) -> u8 {
        if (self.dma_active && !matches!(addr, 0xFF80..=0xFFFE)) || self.cpu_bus_blocked(addr) {
            0xFF
        } else {
            self.read_byte(addr)
        }
    }

    fn cpu_bus_blocked(&self, addr: u16) -> bool {
        if self.read_byte(LCDC_ADDR) & 0x80 == 0 {
            return false;
        }
        match addr {
            0x8000..=0x9FFF => self.ppu_mode == 3,
            0xFE00..=0xFE9F => matches!(self.ppu_mode, 2 | 3),
            _ => false,
        }
    }

    fn cart_ram_index(&self, addr: u16) -> usize {
        let bank = match self.mbc() {
            MbcType::Mbc2 => 0,
            // MBC1 mode 0 is the 16/8 mode: the upper bank register selects
            // ROM bits 5-6 and external RAM remains fixed at bank 0. Only
            // mode 1 exposes the selected RAM bank at A000-BFFF.
            MbcType::Mbc1 if self.banking_mode == 0 => 0,
            _ => self.ram_bank as usize & 0x0F,
        };
        bank * 0x2000 + (addr as usize & 0x1FFF)
    }

    /// Write a byte to the given 16-bit address.
    pub fn write_byte(&mut self, addr: u16, val: u8) {
        // A read-modify-write instruction reads on its next-to-last M-cycle and
        // writes on its last one. The CPU pre-ticks to the read and leaves the
        // remaining M-cycle here, so the write lands on the right cycle.
        if self.pre_write_tick != 0 {
            let pending = std::mem::take(&mut self.pre_write_tick);
            self.tick(pending);
        }
        if addr == LCDC_ADDR && std::env::var_os("FOXMOULATOR_LCD_TRACE").is_some() {
            eprintln!(
                "LCD_TRACE old={:02X} new={val:02X} ly={:02X} mode={} line_cycles={} ticks={} window_line_pending={}",
                self.memory[LCDC_ADDR as usize],
                self.memory[LY_ADDR as usize],
                self.ppu_mode,
                self.ppu_line_cycles,
                self.peripheral_ticks,
                self.hdma_remaining
            );
        }
        if addr >= 0xFF00
            && std::env::var_os("FOXMOULATOR_BOOT_TRACE_MMIO").is_some()
            && self.debug_boot_mmio_events < 512
        {
            eprintln!(
                "BOOT_MMIO_WRITE addr={addr:04X} value={val:02X} ly={:02X} mode={} ticks={}",
                self.memory[LY_ADDR as usize], self.ppu_mode, self.peripheral_ticks
            );
            self.debug_boot_mmio_events += 1;
        }
        match addr {
            0x0000..=0x7FFF => {
                if self.cartridge.is_some() {
                    self.trace_cart_write(addr, val);
                    self.write_mbc_register(addr, val);
                } else {
                    // Without a cartridge the ROM area behaves as plain RAM,
                    // which keeps the standalone unit tests usable.
                    self.memory[addr as usize] = val;
                }
            }
            // Timer registers are owned by the internal Timer.
            DIV_ADDR..=TAC_ADDR => {
                // The APU frame sequencer is derived from DIV. A DIV write
                // can clock it immediately on a falling edge, then resets
                // the phase used for every following event. Keeping the old
                // phase would schedule the next length/sweep/envelope tick
                // too early.
                let apu_frame_event = addr == DIV_ADDR && self.apu_frame_phase >= 4_096;
                self.timer.write_register(addr, val);
                if addr == DIV_ADDR {
                    self.apu_frame_phase = 0;
                }
                if apu_frame_event {
                    self.apu.timer_frame_event();
                }
            }
            // Audio registers and wave RAM are owned by the APU.
            0xFF10..=0xFF3F => {
                if addr == crate::apu::NR52 && val & 0x80 != 0 && !self.apu.enabled {
                    self.apu.set_skip_frame_after_power_on(self.apu_frame_phase);
                }
                self.apu.write_register(addr, val);
            }
            // Writing P1 selects the button group and may request an interrupt.
            JOYP_ADDR => {
                if std::env::var_os("FOXMOULATOR_JOYP_TRACE").is_some() {
                    eprintln!(
                        "JOYP_TRACE write={val:02X} read_before={:02X} ly={:02X} mode={} ticks={}",
                        self.joypad.read(),
                        self.memory[LY_ADDR as usize],
                        self.ppu_mode,
                        self.peripheral_ticks
                    );
                }
                if self.joypad.write(val) {
                    let if_reg = self.memory[IF_ADDR as usize];
                    self.memory[IF_ADDR as usize] = if_reg | 0x10;
                }
            }
            // Serial data (SB) and control (SC).
            0xFF01..=0xFF02 => self.serial.write_register(addr, val),
            // Interrupt flag: only bits 0-4 are writable; the unused upper
            // bits read back as 1.
            IF_ADDR => self.memory[IF_ADDR as usize] = val & 0x1F,
            // A non-zero write permanently unmaps the boot ROM.
            0xFF50 => {
                if val != 0 {
                    self.boot_rom_enabled = false;
                }
                self.memory[addr as usize] = val;
            }
            // KEY1: only the speed-switch arm bit is writable on CGB.
            0xFF4D => {
                if self.cgb {
                    self.key1_armed = (val & 0x01) != 0;
                }
            }
            VBK_ADDR => {
                if self.cgb {
                    self.vram_bank = val & 1;
                }
            }
            SVBK_ADDR => {
                if self.cgb {
                    self.wram_bank = match val & 7 {
                        0 => 1,
                        bank => bank,
                    };
                }
            }
            DMA_ADDR => {
                self.dma_source = val;
                self.dma_active = true;
                self.dma_index = 0;
                self.dma_phase = 0;
            }
            HDMA1_ADDR => {
                if self.cgb {
                    self.hdma_source = (u16::from(val) << 8) | (self.hdma_source & 0x00F0);
                }
            }
            HDMA2_ADDR => {
                if self.cgb {
                    self.hdma_source = (self.hdma_source & 0xFF00) | u16::from(val & 0xF0);
                }
            }
            HDMA3_ADDR => {
                if self.cgb {
                    self.hdma_destination =
                        0x8000 | ((u16::from(val & 0x1F)) << 8) | (self.hdma_destination & 0x00F0);
                }
            }
            HDMA4_ADDR => {
                if self.cgb {
                    self.hdma_destination =
                        0x8000 | (self.hdma_destination & 0x1F00) | u16::from(val & 0xF0);
                }
            }
            HDMA5_ADDR => {
                if self.cgb {
                    if self.hdma_active && self.hdma_hblank_mode && val & 0x80 == 0 {
                        self.hdma_active = false;
                        self.hdma_hblank_mode = false;
                    } else {
                        self.hdma_remaining = (val & 0x7F) + 1;
                        self.hdma_hblank_mode = val & 0x80 != 0;
                        self.hdma_active = true;
                        if !self.hdma_hblank_mode {
                            let blocks = self.hdma_remaining;
                            for _ in 0..blocks {
                                self.copy_hdma_block();
                            }
                            self.tick(u32::from(blocks) * 32);
                        }
                    }
                }
            }
            BCPS_ADDR => {
                if self.cgb {
                    self.cgb_bg_palette_index = val & 0xBF;
                }
            }
            BCPD_ADDR => {
                if self.cgb {
                    let index = (self.cgb_bg_palette_index & 0x3F) as usize;
                    if std::env::var_os("FOXMOULATOR_PALETTE_TRACE").is_some() {
                        eprintln!(
                            "PALETTE_TRACE kind=bg data={val:02X} index={index:02X} ly={:02X} mode={} ticks={}",
                            self.memory[LY_ADDR as usize],
                            self.ppu_mode,
                            self.peripheral_ticks
                        );
                    }
                    self.cgb_bg_palette[index] = val;
                    if self.cgb_bg_palette_index & 0x80 != 0 {
                        self.cgb_bg_palette_index = 0x80 | ((index as u8 + 1) & 0x3F);
                    }
                }
            }
            OCPS_ADDR => {
                if self.cgb {
                    self.cgb_obj_palette_index = val & 0xBF;
                }
            }
            OCPD_ADDR => {
                if self.cgb {
                    let index = (self.cgb_obj_palette_index & 0x3F) as usize;
                    if std::env::var_os("FOXMOULATOR_PALETTE_TRACE").is_some() {
                        eprintln!(
                            "PALETTE_TRACE kind=obj data={val:02X} index={index:02X} ly={:02X} mode={} ticks={}",
                            self.memory[LY_ADDR as usize],
                            self.ppu_mode,
                            self.peripheral_ticks
                        );
                    }
                    self.cgb_obj_palette[index] = val;
                    if self.cgb_obj_palette_index & 0x80 != 0 {
                        self.cgb_obj_palette_index = 0x80 | ((index as u8 + 1) & 0x3F);
                    }
                }
            }
            OPRI_ADDR => {
                if self.cgb {
                    self.opri = val & 1;
                }
            }
            0x8000..=0x9FFF => self.write_vram_byte(addr, val),
            0xC000..=0xDFFF => self.write_wram_byte(addr, val),
            0xFE00..=0xFE9F | 0xFF00..=0xFFFE => self.memory[addr as usize] = val,
            0xA000..=0xBFFF => {
                if self.cartridge.is_none() {
                    self.memory[addr as usize] = val;
                    return;
                }
                if !self.ram_enabled {
                    return;
                }
                let index = self.cart_ram_index(addr);
                match self.mbc() {
                    MbcType::Mbc2 => {
                        if let Some(slot) = self.external_ram.get_mut(index) {
                            *slot = val & 0x0F;
                        }
                    }
                    MbcType::Mbc3 if self.ram_bank >= 0x08 => {
                        let register = (self.ram_bank - 0x08) as usize;
                        self.update_rtc_wall_clock();
                        self.rtc_registers[register] = match register {
                            0 => val % 60,
                            1 => val % 60,
                            2 => val % 24,
                            3 => val,
                            4 => val & 0xC1,
                            _ => unreachable!(),
                        };
                        self.rtc_last_unix = Self::unix_now();
                    }
                    _ => {
                        if let Some(slot) = self.external_ram.get_mut(index) {
                            *slot = val;
                        }
                    }
                }
                self.trace_cart_write(addr, val);
            }
            // Echo RAM writes go to WRAM.
            0xE000..=0xFDFF => self.write_wram_byte(addr - 0x2000, val),
            // Unusable region: writes are ignored.
            0xFEA0..=0xFEFF => {}
            // IE.
            0xFFFF => self.memory[IE_ADDR as usize] = val,
        }
    }

    /// Write through the CPU bus, ignoring writes to VRAM/OAM while the PPU
    /// owns those buses. The pending timing tick is still consumed, even when
    /// the data write is blocked.
    pub fn cpu_write_byte(&mut self, addr: u16, val: u8) {
        let trace_vram = (0x8000..=0x81FF).contains(&addr)
            && !self.boot_rom_enabled
            && std::env::var_os("FOXMOULATOR_VRAM_TRACE").is_some()
            && (val != 0 || addr < 0x8020)
            && self.debug_vram_events < 600;
        if self.dma_active && !matches!(addr, 0xFF80..=0xFFFE) {
            if self.pre_write_tick != 0 {
                let pending = std::mem::take(&mut self.pre_write_tick);
                self.tick(pending);
            }
            if self.dma_active {
                if trace_vram {
                    eprintln!(
                        "VRAM_WRITE addr={addr:04X} value={val:02X} result=blocked-dma ly={:02X} mode={} ticks={}",
                        self.memory[LY_ADDR as usize], self.ppu_mode, self.peripheral_ticks
                    );
                    self.debug_vram_events += 1;
                }
                return;
            }
        }
        if self.cpu_bus_blocked(addr) {
            if self.pre_write_tick != 0 {
                let pending = std::mem::take(&mut self.pre_write_tick);
                self.tick(pending);
            }
            if trace_vram {
                eprintln!(
                    "VRAM_WRITE addr={addr:04X} value={val:02X} result=blocked-ppu ly={:02X} mode={} ticks={}",
                    self.memory[LY_ADDR as usize], self.ppu_mode, self.peripheral_ticks
                );
                self.debug_vram_events += 1;
            }
            return;
        }
        if trace_vram {
            eprintln!(
                "VRAM_WRITE addr={addr:04X} value={val:02X} result=written ly={:02X} mode={} ticks={}",
                self.memory[LY_ADDR as usize], self.ppu_mode, self.peripheral_ticks
            );
            self.debug_vram_events += 1;
        }
        self.write_byte(addr, val);
    }

    fn write_mbc_register(&mut self, addr: u16, val: u8) {
        let mbc = self.mbc();
        match mbc {
            MbcType::RomOnly => {}
            MbcType::Mbc1 => match addr {
                0x0000..=0x1FFF => self.ram_enabled = (val & 0x0F) == 0x0A,
                0x2000..=0x3FFF => {
                    let low = (val & 0x1F) as u16;
                    let low = if low == 0 { 1 } else { low };
                    self.current_rom_bank = (self.current_rom_bank & 0x60) | low;
                    self.fix_rom_bank();
                }
                0x4000..=0x5FFF => {
                    if self.banking_mode == 0 {
                        self.current_rom_bank =
                            (self.current_rom_bank & 0x1F) | (((val & 0x03) as u16) << 5);
                        self.fix_rom_bank();
                    } else {
                        self.ram_bank = val & 0x03;
                    }
                }
                0x6000..=0x7FFF => {
                    self.banking_mode = val & 0x01;
                    if self.banking_mode == 0 {
                        // In mode 0 the upper bits belong to the ROM bank.
                        self.current_rom_bank &= 0x1F;
                        self.fix_rom_bank();
                    }
                }
                _ => {}
            },
            MbcType::Mbc2 => {
                if addr <= 0x3FFF {
                    if (addr & 0x0100) == 0 {
                        self.ram_enabled = (val & 0x0F) == 0x0A;
                    } else {
                        let bank = (val & 0x0F) as u16;
                        self.current_rom_bank = if bank == 0 { 1 } else { bank };
                        self.fix_rom_bank();
                    }
                }
            }
            MbcType::Mbc3 => match addr {
                0x0000..=0x1FFF => self.ram_enabled = (val & 0x0F) == 0x0A,
                0x2000..=0x3FFF => {
                    let bank = (val & 0x7F) as u16;
                    self.current_rom_bank = if bank == 0 { 1 } else { bank };
                    self.fix_rom_bank();
                }
                0x4000..=0x5FFF => self.ram_bank = val,
                0x6000..=0x7FFF => {
                    self.update_rtc_wall_clock();
                    if self.rtc_latch == 0 && val == 1 {
                        self.rtc_latched = self.rtc_registers;
                    }
                    self.rtc_latch = val;
                }
                _ => {}
            },
            MbcType::Mbc5 => match addr {
                0x0000..=0x1FFF => self.ram_enabled = (val & 0x0F) == 0x0A,
                0x2000..=0x2FFF => {
                    self.current_rom_bank = (self.current_rom_bank & 0x100) | val as u16;
                    self.fix_rom_bank();
                }
                0x3000..=0x3FFF => {
                    self.current_rom_bank =
                        (self.current_rom_bank & 0x0FF) | (((val & 0x01) as u16) << 8);
                    self.fix_rom_bank();
                }
                0x4000..=0x5FFF => self.ram_bank = val & 0x0F,
                _ => {}
            },
        }
    }

    /// Apply mapper-specific selected-ROM-bank rules.
    /// Bank 0 of the switchable region maps to bank 1 on MBC1/2/3.
    fn fix_rom_bank(&mut self) {
        // MBC registers retain their selected address lines even when a
        // cartridge does not populate that ROM bank. Reducing the register
        // modulo the loaded image aliases (for example) MBC1 bank 4 to bank
        // 1 on a four-bank cartridge, which can turn a table sentinel into
        // executable data. `rom_byte` returns open-bus 0xFF for an absent
        // physical bank instead.
        if self.current_rom_bank == 0 {
            self.current_rom_bank = 1;
        }
        // This legacy normalization remains correct for the other mapper
        // implementations in this MMU. MBC1 must retain its external address
        // lines, handled above, so a sparse physical bank reads as open bus.
        if self.mbc() != MbcType::Mbc1 {
            self.current_rom_bank %= self.rom_bank_count() as u16;
            if self.current_rom_bank == 0 {
                self.current_rom_bank = 1;
            }
        }
    }

    /// Load a ROM or other binary data into memory starting at `start_addr`.
    /// Panics if the data would overflow the 64 KiB address space.
    pub fn load_rom(&mut self, start_addr: u16, data: &[u8]) {
        let start = start_addr as usize;
        let end = start + data.len();
        if end > self.memory.len() {
            panic!("ROM data exceeds address space");
        }
        self.memory[start..end].copy_from_slice(data);
    }

    /// Load an entire cartridge and initialise MBC state from its header.
    pub fn load_cartridge(&mut self, rom_data: Vec<u8>) {
        let cart = Cartridge::new(rom_data);
        let ram_size = cart.ram_size.max(0x2000);
        let ram_size = if cart.mbc == MbcType::Mbc2 {
            512
        } else {
            ram_size
        };
        self.cartridge = Some(cart);
        self.ram_enabled = false;
        self.current_rom_bank = 1;
        self.banking_mode = 0;
        self.ram_bank = 0;
        self.rtc_latch = 0;
        self.rtc_registers = [0; 5];
        self.rtc_latched = [0; 5];
        self.rtc_last_unix = Self::unix_now();
        self.external_ram = vec![0; ram_size];
    }

    /// Map one of FoxMulator's embedded SameBoy boot ROMs.
    pub(crate) fn load_integrated_boot_rom(
        &mut self,
        boot_rom: &'static [u8],
    ) -> Result<(), String> {
        let expected = if self.cgb { 0x900 } else { 0x100 };
        if boot_rom.len() != expected {
            return Err(format!(
                "invalid {} boot ROM size: expected {expected}, got {}",
                if self.cgb { "CGB" } else { "DMG" },
                boot_rom.len()
            ));
        }
        self.boot_rom = Some(boot_rom);
        self.boot_rom_enabled = true;
        self.memory[0xFF50] = 0;
        Ok(())
    }

    /// Advance peripherals by `cycles` CPU T-cycles.
    ///
    /// In CGB double-speed mode the CPU clock doubles. The divider and timer
    /// are clocked by the CPU clock, so they keep the same rate relative to
    /// instructions; the system clock (PPU, APU, serial) stays at the normal
    /// rate, so those peripherals receive half as many ticks. The fractional
    /// cycles are kept across calls. Reference: Pan Docs, "Double Speed Mode".
    pub fn tick(&mut self, cycles: u32) {
        self.update_rtc_wall_clock();
        let mut system_cycles = cycles;
        if self.double_speed {
            self.speed_accum += cycles;
            system_cycles = self.speed_accum / 2;
            self.speed_accum -= system_cycles * 2;
        }
        self.peripheral_ticks = self.peripheral_ticks.wrapping_add(u64::from(system_cycles));
        if self.dma_active {
            let dma_total = u32::from(self.dma_phase) + system_cycles;
            let bytes = (dma_total / 4).min(u32::from(0xA0 - self.dma_index));
            self.dma_phase = (dma_total % 4) as u8;
            for _ in 0..bytes {
                self.copy_oam_dma_byte();
            }
        }
        let phase_total = self.apu_frame_phase + system_cycles;
        let frame_events = phase_total / 8_192;
        self.apu_frame_phase = phase_total % 8_192;
        // Mirror the system clock into the scanline position so intra-instruction
        // accesses can be checked against the PPU's OAM scan row.
        self.advance_ppu_clock(system_cycles);
        // Divider/TIMA run from the (possibly doubled) CPU clock.
        self.timer.tick(cycles);
        if self.timer.take_interrupt() {
            let if_reg = self.read_byte(IF_ADDR);
            self.write_byte(IF_ADDR, if_reg | 0x04);
        }
        if self.serial.tick(system_cycles) {
            let if_reg = self.read_byte(IF_ADDR);
            self.write_byte(IF_ADDR, if_reg | 0x08);
        }
        self.apu.tick_with_frame_events(system_cycles, frame_events);
    }

    /// Complete one byte transfer driven by an external serial clock.
    /// Returns whether an external-clock transfer was completed.
    pub fn clock_external_serial(&mut self, received: u8) -> bool {
        if !self.serial.clock_external(received) {
            return false;
        }
        let if_reg = self.read_byte(IF_ADDR);
        self.write_byte(IF_ADDR, if_reg | 0x08);
        true
    }

    /// Reset the mirrored PPU clock to the state right after `Ppu::reset`.
    ///
    /// When the LCD is already on (the usual post-boot state) the first line is
    /// a normal 456-cycle line. A later off-to-on transition is handled by
    /// [`Mmu::advance_ppu_clock`] and uses the shorter 452-cycle first line.
    pub fn reset_ppu_clock(&mut self) {
        let lcd_on = (self.read_byte(LCDC_ADDR) & 0x80) != 0;
        self.ppu_lcd_on = lcd_on;
        self.ppu_first_line = false;
        self.ppu_line_cycles = 0;
        self.ppu_mode = if lcd_on { 2 } else { 0 };
        self.oam_search_row = 0;
        self.oam_search_phase = 0;
        self.memory[LY_ADDR as usize] = 0;
        let stat = self.memory[STAT_ADDR as usize];
        self.memory[STAT_ADDR as usize] = (stat & 0x7C) | self.ppu_mode;
    }

    /// Advance the mirrored PPU scanline clock by `cycles` T-cycles.
    ///
    /// The PPU consumes this clock (see [`crate::ppu::Ppu::tick`]) so there is a
    /// single source of truth for LY, mode and STAT. The CPU calls it per
    /// M-cycle, which lets OAM accesses be checked against the row the PPU is
    /// actually scanning. Reference: Pan Docs, "Rendering"; SameBoy
    /// `display.c` (`GB_display_run`).
    pub fn advance_ppu_clock(&mut self, cycles: u32) {
        let lcd_on = (self.read_byte(LCDC_ADDR) & 0x80) != 0;
        if !lcd_on {
            self.ppu_lcd_on = false;
            self.ppu_first_line = false;
            self.ppu_line_cycles = 0;
            self.ppu_mode = 0;
            self.oam_search_row = 0;
            self.oam_search_phase = 0;
            return;
        }

        if !self.ppu_lcd_on {
            // Off-to-on transition: the first visible line is one M-cycle
            // shorter than usual (452 rather than 456 T-cycles).
            self.ppu_lcd_on = true;
            self.ppu_first_line = true;
            self.ppu_line_cycles = 0;
            self.memory[LY_ADDR as usize] = 0;
            self.ppu_mode = 2;
            self.oam_search_row = 0;
            self.oam_search_phase = 0;
        }

        self.ppu_line_cycles = self.ppu_line_cycles.wrapping_add(cycles);
        // The DMG OAM scanner selects a pair of objects every two M-cycles.
        // Keep the selected row as state rather than deriving it at access
        // time; this also gives LCD transitions a well-defined initial row.
        let scan_cycle = self.ppu_line_cycles % crate::ppu::SCANLINE_CYCLES;
        if scan_cycle < 80 {
            // OAM search advances one object pair per M-cycle (4 T-cycles).
            self.oam_search_phase = scan_cycle / 4;
            self.oam_search_row = (self.oam_search_phase as usize) * 8;
        } else {
            self.oam_search_phase = 0;
            self.oam_search_row = 0;
        }
        // LY is observable by the CPU during the data-access M-cycle. Mirror
        // the first LCD-on transition here rather than waiting for the PPU
        // presentation step at the end of the instruction.
        if self.ppu_first_line && self.ppu_line_cycles >= 452 {
            self.memory[LY_ADDR as usize] = 1;
        }
        self.ppu_mode = if self.memory[LY_ADDR as usize] >= crate::ppu::VISIBLE_LINES {
            1
        } else if self.ppu_line_cycles < 80 {
            2
        } else if self.ppu_line_cycles < 252 {
            3
        } else {
            0
        };
    }

    /// OAM byte offset (0, 8, 16, ... 152) the PPU is currently scanning, or
    /// `None` when the DMG OAM corruption bug cannot be triggered (CGB, LCD
    /// off, VBlank, or outside mode 2). Reference: Pan Docs, "OAM Corruption
    /// Bug"; SameBoy `accessed_oam_row`.
    fn oam_bug_row(&self) -> Option<usize> {
        if self.cgb {
            return None;
        }
        if (self.read_byte(LCDC_ADDR) & 0x80) == 0 {
            return None;
        }
        let total = self.ppu_line_cycles as u64;
        // `ppu_line_cycles` is normally wrapped by the PPU, but an access can
        // cross the end of a line before the PPU processes it.
        let wraps = total / u64::from(crate::ppu::SCANLINE_CYCLES);
        let cycle = (total % u64::from(crate::ppu::SCANLINE_CYCLES)) as u32;
        let ly = ((self.read_byte(LY_ADDR) as u64 + wraps) % 154) as u8;
        if ly >= 144 || cycle >= 80 {
            return None;
        }
        Some(self.oam_search_row)
    }

    fn oam_word(&self, offset: usize) -> u16 {
        let lo = self.memory[0xFE00 + offset];
        let hi = self.memory[0xFE00 + offset + 1];
        u16::from_le_bytes([lo, hi])
    }

    fn set_oam_word(&mut self, offset: usize, value: u16) {
        self.memory[0xFE00 + offset] = value as u8;
        self.memory[0xFE00 + offset + 1] = (value >> 8) as u8;
    }

    /// Apply a write-caused OAM corruption (16-bit increment/decrement or an
    /// OAM write while the PPU is scanning OAM).
    ///
    /// Objects 0 and 1 (row 0) are not affected. Pan Docs, "Write Corruption".
    pub fn oam_bug_write(&mut self) {
        let Some(row) = self.oam_bug_row() else {
            return;
        };
        if row < 8 {
            return;
        }
        let a = self.oam_word(row);
        let b = self.oam_word(row - 8);
        let c = self.oam_word(row - 4);
        let value = ((a ^ c) & (b ^ c)) ^ c;
        self.set_oam_word(row, value);
        for i in 2..8 {
            self.memory[0xFE00 + row + i] = self.memory[0xFE00 + row - 8 + i];
        }
        if std::env::var_os("FOX_OAM_TRACE").is_some() {
            eprintln!(
                "OAM_EVENT kind=write branch=write-glitch row={row:02X} writes=word-current+copy-preceding simultaneous=false"
            );
        }
    }

    /// Apply a read-caused OAM corruption (CPU read from OAM while the PPU is
    /// scanning OAM). Pan Docs, "Read Corruption".
    pub fn oam_bug_read(&mut self) {
        self.oam_bug_read_context(None);
    }

    /// Read-corruption entry point with CPU bus context for differential
    /// diagnostics. The context is deliberately optional so ordinary OAM
    /// accesses retain the same emulation path.
    pub fn oam_bug_read_context(&mut self, context: Option<(u16, u16, &'static str)>) {
        let Some(row) = self.oam_bug_row() else {
            return;
        };
        if row < 8 {
            return;
        }
        let before = if std::env::var_os("FOX_OAM_TRACE").is_some() {
            Some((self.oam_trace_window(row), self.oam_trace_dump()))
        } else {
            None
        };
        let a = self.oam_word(row);
        let b = self.oam_word(row - 8);
        let c = self.oam_word(row - 4);
        let value = b | (a & c);
        self.set_oam_word(row - 8, value);
        self.set_oam_word(row, value);
        for i in 2..8 {
            self.memory[0xFE00 + row + i] = self.memory[0xFE00 + row - 8 + i];
        }
        if let Some((before, before_full)) = before {
            let after = self.oam_trace_window(row);
            let after_full = self.oam_trace_dump();
            let context = context
                .map(|(address, sp, kind)| format!("kind={kind} addr={address:04X} sp={sp:04X}"))
                .unwrap_or_else(|| "kind=read addr=---- sp=----".to_string());
            eprintln!(
                "OAM_CORRUPTION {context} branch=read-normal-local value={value:04X} writes=word-preceding+word-current+copy-preceding simultaneous=false tick={} phase={} ly={} line_cycles={} mode={} row={row:02X} before={before} after={after} before_oam={before_full} after_oam={after_full}",
                self.peripheral_ticks,
                self.peripheral_ticks % 4,
                self.read_byte(LY_ADDR),
                self.ppu_line_cycles,
                self.ppu_mode
            );
        }
    }

    fn oam_trace_window(&self, row: usize) -> String {
        let start = row.saturating_sub(8);
        (start..(row + 8).min(0xA0))
            .map(|offset| format!("{:02X}", self.memory[0xFE00 + offset]))
            .collect::<Vec<_>>()
            .join("")
    }

    fn oam_trace_dump(&self) -> String {
        (0..0xA0)
            .map(|offset| format!("{:02X}", self.memory[0xFE00 + offset]))
            .collect::<Vec<_>>()
            .join("")
    }

    /// Apply the read-during-increment corruption used by POP and by
    /// auto-incrementing OAM reads. The first stack read and SP increment share
    /// one M-cycle on the hardware, so this is distinct from a plain read
    /// corruption. Reference: Pan Docs, "Read During Increase/Decrease" and
    /// SameBoy's `GB_trigger_oam_bug_read_increase`.
    pub fn oam_bug_read_increase(&mut self) {
        let Some(row) = self.oam_bug_row() else {
            return;
        };
        // The mixed read/write pattern is undefined for the first four rows
        // and the final row of the OAM scan.
        if !(0x20..0x98).contains(&row) {
            return;
        }

        let a = self.oam_word(row - 0x10);
        let b = self.oam_word(row - 0x08);
        let c = self.oam_word(row);
        let d = self.oam_word(row - 0x04);
        let value = (b & (a | c | d)) | (a & c & d);

        let a1 = self.oam_word(row - 0x0F);
        let b1 = self.oam_word(row - 0x07);
        let c1 = self.oam_word(row + 0x01);
        let d1 = self.oam_word(row - 0x03);
        let value1 = (b1 & (a1 | c1 | d1)) | (a1 & c1 & d1);

        self.set_oam_word(row - 0x08, value);
        self.set_oam_word(row - 0x07, value1);
        // The preceding row is copied to both the currently scanned row and
        // the row two positions before it after its first word is glitched.
        for i in 0..8 {
            let byte = self.memory[0xFE00 + row - 0x08 + i];
            self.memory[0xFE00 + row + i] = byte;
            self.memory[0xFE00 + row - 0x10 + i] = byte;
        }
        if std::env::var_os("FOX_OAM_TRACE").is_some() {
            eprintln!(
                "OAM_EVENT kind=read-increase branch=read-increase row={row:02X} writes=two-preceding+current simultaneous=false"
            );
        }
    }

    /// Diagnostic snapshot for cycle-level OAM investigations. This does not
    /// participate in emulation and is intentionally read-only.
    pub fn oam_debug_state(&self) -> (u64, u32, u8, usize) {
        (
            self.peripheral_ticks,
            self.ppu_line_cycles,
            self.ppu_mode,
            self.oam_search_row,
        )
    }

    /// Press or release a button and request the joypad interrupt on a
    /// high-to-low transition.
    pub fn set_button(&mut self, button: crate::joypad::Button, pressed: bool) {
        if std::env::var_os("FOXMOULATOR_JOYP_TRACE").is_some() {
            eprintln!(
                "JOYP_TRACE input={button:?} pressed={pressed} read_before={:02X} ly={:02X} mode={} ticks={}",
                self.joypad.read(),
                self.memory[LY_ADDR as usize],
                self.ppu_mode,
                self.peripheral_ticks
            );
        }
        if self.joypad.set_button(button, pressed) {
            let if_reg = self.memory[IF_ADDR as usize];
            self.memory[IF_ADDR as usize] = if_reg | 0x10;
        }
    }

    /// Current switchable ROM bank (useful for tests and debugging).
    pub fn rom_bank(&self) -> u16 {
        self.current_rom_bank
    }

    /// True when the loaded cartridge has battery-backed save RAM.
    ///
    /// The cartridge header byte 0x0147 encodes the MBC and battery flag. The
    /// header type table in Pan Docs lists which types carry a battery.
    pub fn has_battery(&self) -> bool {
        let cart = match &self.cartridge {
            Some(cart) => cart,
            None => return false,
        };
        let header_type = cart.rom.get(0x0147).copied().unwrap_or(0x00);
        matches!(
            header_type,
            // MBC1+RAM+BATTERY, MBC2+BATTERY, MBC3+RAM+BATTERY variants and
            // MBC5+RAM+BATTERY variants.
            0x03 | 0x06 | 0x09 | 0x0D | 0x0F | 0x10 | 0x13 | 0x1B | 0x1E
        )
    }

    /// Raw battery-backed RAM contents.
    pub fn export_ram(&self) -> Vec<u8> {
        self.external_ram.clone()
    }

    /// Restore battery-backed RAM contents. The data is truncated or
    /// zero-padded to the current save-RAM size.
    pub fn import_ram(&mut self, data: &[u8]) {
        let size = self.external_ram.len();
        self.external_ram.clear();
        self.external_ram.resize(size, 0);
        let copy = data.len().min(size);
        self.external_ram[..copy].copy_from_slice(&data[..copy]);
    }

    /// Write battery-backed RAM (and MBC3 RTC state when present) atomically.
    /// Raw SRAM files remain unchanged for non-RTC cartridges.
    pub fn save_ram(&mut self, path: &std::path::Path) -> std::io::Result<()> {
        use std::io::Write;
        self.update_rtc_wall_clock();
        let temp = path.with_extension("sav.tmp");
        {
            let mut file = std::fs::File::create(&temp)?;
            if self.has_rtc_chip() {
                file.write_all(b"FOXRTC1\0")?;
                file.write_all(&(self.external_ram.len() as u32).to_le_bytes())?;
                file.write_all(&self.external_ram)?;
                file.write_all(&self.rtc_registers)?;
                file.write_all(&self.rtc_latched)?;
                file.write_all(&self.rtc_last_unix.to_le_bytes())?;
            } else {
                file.write_all(&self.external_ram)?;
            }
            file.sync_all()?;
        }
        std::fs::rename(&temp, path)
    }

    /// Load battery-backed RAM from `path`. Missing files are treated as a
    /// fresh (zeroed) save and return `Ok(false)`.
    pub fn load_ram(&mut self, path: &std::path::Path) -> std::io::Result<bool> {
        match std::fs::read(path) {
            Ok(data) => {
                if self.has_rtc_chip() && data.starts_with(b"FOXRTC1\0") {
                    use std::io::{Error, ErrorKind};
                    const HEADER: usize = 8 + 4;
                    const TAIL: usize = 5 + 5 + 8;
                    if data.len() < HEADER + TAIL {
                        return Err(Error::new(ErrorKind::InvalidData, "truncated RTC save"));
                    }
                    let ram_len = u32::from_le_bytes(data[8..12].try_into().unwrap()) as usize;
                    if data.len() != HEADER + ram_len + TAIL {
                        return Err(Error::new(ErrorKind::InvalidData, "invalid RTC save size"));
                    }
                    self.import_ram(&data[HEADER..HEADER + ram_len]);
                    self.rtc_registers
                        .copy_from_slice(&data[HEADER + ram_len..HEADER + ram_len + 5]);
                    self.rtc_latched
                        .copy_from_slice(&data[HEADER + ram_len + 5..HEADER + ram_len + 10]);
                    self.rtc_last_unix =
                        u64::from_le_bytes(data[HEADER + ram_len + 10..].try_into().unwrap());
                    self.update_rtc_wall_clock();
                } else {
                    // Backward compatibility with existing raw SRAM files.
                    self.import_ram(&data);
                    self.rtc_last_unix = Self::unix_now();
                }
                Ok(true)
            }
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(false),
            Err(err) => Err(err),
        }
    }
}
