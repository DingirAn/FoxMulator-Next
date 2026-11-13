//! Cartridge header parsing and MBC type detection.

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum MbcType {
    RomOnly,
    Mbc1,
    Mbc2,
    Mbc3,
    Mbc5,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum RomType {
    GBC,
    DMG,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Cartridge {
    pub rom: Vec<u8>,
    pub rom_size: usize,
    pub ram_size: usize,
    pub mbc: MbcType,
    pub rom_type: RomType,
}

impl Cartridge {
    pub fn new(rom_data: Vec<u8>) -> Self {
        // ROM type from header byte 0x0143.
        let rom_type = match rom_data.get(0x0143).copied().unwrap_or(0x00) {
            0x80 | 0xC0 => RomType::GBC,
            _ => RomType::DMG,
        };

        let mbc = match rom_data.get(0x0147).copied().unwrap_or(0x00) {
            0x00 => MbcType::RomOnly,
            0x01..=0x03 => MbcType::Mbc1,
            0x05 | 0x06 => MbcType::Mbc2,
            0x0F..=0x13 => MbcType::Mbc3,
            0x19..=0x1E => MbcType::Mbc5,
            _ => MbcType::RomOnly,
        };

        // External RAM size from header byte 0x0149 (Pan Docs).
        let ram_size = match rom_data.get(0x0149).copied().unwrap_or(0x00) {
            0x01 => 0x0800,  // 2 KiB
            0x02 => 0x2000,  // 8 KiB
            0x03 => 0x8000,  // 32 KiB
            0x04 => 0x20000, // 128 KiB
            0x05 => 0x10000, // 64 KiB
            _ => 0,
        };

        Self {
            rom_size: rom_data.len(),
            ram_size,
            rom: rom_data,
            mbc,
            rom_type,
        }
    }

    /// Number of 16 KiB ROM banks, rounded up to at least one.
    pub fn rom_bank_count(&self) -> usize {
        (self.rom.len() / 0x4000).max(1)
    }
}
