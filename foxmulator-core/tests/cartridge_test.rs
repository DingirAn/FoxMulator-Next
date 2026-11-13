#[cfg(test)]
mod tests {
    use foxmulator_core::cartridge::{Cartridge, MbcType, RomType};

    #[test]
    fn test_mbc_detection() {
        let mut data = vec![0u8; 0x150];
        data[0x0147] = 0x01; // MBC1
        data[0x0143] = 0x80; // GBC
        let cartridge = Cartridge::new(data);
        assert_eq!(cartridge.mbc, MbcType::Mbc1);
        assert_eq!(cartridge.rom_type, RomType::GBC);
    }
}
