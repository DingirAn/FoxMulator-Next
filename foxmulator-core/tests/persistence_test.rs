//! Battery-backed save RAM persistence tests.

#[cfg(test)]
mod tests {
    use foxmulator_core::mmu::Mmu;
    use std::path::PathBuf;

    fn cart_rom(header_type: u8, ram_code: u8) -> Vec<u8> {
        let mut rom = vec![0u8; 0x8000];
        rom[0x0147] = header_type;
        rom[0x0149] = ram_code;
        rom
    }

    #[test]
    fn battery_detection_follows_header() {
        let mut mmu = Mmu::new();
        mmu.load_cartridge(cart_rom(0x03, 0x02)); // MBC1+RAM+BATTERY
        assert!(mmu.has_battery());

        mmu.load_cartridge(cart_rom(0x01, 0x02)); // MBC1 without battery
        assert!(!mmu.has_battery());

        mmu.load_cartridge(cart_rom(0x1B, 0x03)); // MBC5+RAM+BATTERY
        assert!(mmu.has_battery());
    }

    #[test]
    fn ram_roundtrip_in_memory() {
        let mut mmu = Mmu::new();
        mmu.load_cartridge(cart_rom(0x03, 0x02)); // 8 KiB save RAM
        mmu.ram_enabled = true;
        mmu.write_byte(0xA000, 0x12);
        mmu.write_byte(0xA123, 0x34);

        let saved = mmu.export_ram();
        assert_eq!(saved.len(), 0x2000);
        assert_eq!(saved[0x0000], 0x12);
        assert_eq!(saved[0x0123], 0x34);

        let mut other = Mmu::new();
        other.load_cartridge(cart_rom(0x03, 0x02));
        other.import_ram(&saved);
        other.ram_enabled = true;
        assert_eq!(other.read_byte(0xA000), 0x12);
        assert_eq!(other.read_byte(0xA123), 0x34);
    }

    #[test]
    fn import_pads_and_truncates() {
        let mut mmu = Mmu::new();
        mmu.load_cartridge(cart_rom(0x03, 0x02));
        mmu.import_ram(&[1, 2, 3]);
        assert_eq!(&mmu.export_ram()[..4], &[1, 2, 3, 0]);
        assert_eq!(mmu.export_ram().len(), 0x2000);
    }

    fn temp_path(name: &str) -> PathBuf {
        let mut path = std::env::temp_dir();
        path.push(format!(
            "foxmulator-{name}-{}-{}.sav",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        path
    }

    #[test]
    fn save_and_load_ram_file() {
        let path = temp_path("persist");
        let mut mmu = Mmu::new();
        mmu.load_cartridge(cart_rom(0x03, 0x02));
        mmu.ram_enabled = true;
        mmu.write_byte(0xA000, 0xAB);
        mmu.write_byte(0xBFFF, 0xCD);

        mmu.save_ram(&path).expect("save succeeds");
        assert!(path.exists());

        let mut loaded = Mmu::new();
        loaded.load_cartridge(cart_rom(0x03, 0x02));
        assert!(loaded.load_ram(&path).expect("load succeeds"));
        assert_eq!(loaded.export_ram()[0], 0xAB);
        assert_eq!(loaded.export_ram()[0x1FFF], 0xCD);

        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn missing_save_file_is_not_an_error() {
        let path = temp_path("missing");
        let mut mmu = Mmu::new();
        mmu.load_cartridge(cart_rom(0x03, 0x02));
        assert!(!mmu.load_ram(&path).expect("missing file is ok"));
    }

    #[test]
    fn mbc3_rtc_advances_halts_latches_and_wraps() {
        let mut mmu = Mmu::new();
        mmu.load_cartridge(cart_rom(0x10, 0x03)); // MBC3 + RAM + RTC + battery
        mmu.ram_enabled = true;

        mmu.advance_rtc_seconds(86_400 + 3_661);
        assert_eq!(mmu.rtc_state(), [1, 1, 1, 1, 0]);

        // Halt freezes the live clock, while the latch exposes a stable copy.
        mmu.ram_bank = 0x0C;
        mmu.write_byte(0xA000, 0x40);
        mmu.advance_rtc_seconds(60);
        mmu.write_byte(0x6000, 0);
        mmu.write_byte(0x6000, 1);
        assert_eq!(mmu.read_byte(0xA000), 0x40);

        // Clear halt and cross the 512-day boundary.
        mmu.write_byte(0xA000, 0);
        mmu.rtc_registers = [59, 59, 23, 0xFF, 0x01];
        mmu.advance_rtc_seconds(1);
        assert_eq!(mmu.rtc_state(), [0, 0, 0, 0, 0x80]);
    }

    #[test]
    fn mbc3_rtc_save_roundtrip_preserves_ram_clock_and_legacy_compatibility() {
        let path = temp_path("rtc-persist");
        let mut mmu = Mmu::new();
        mmu.load_cartridge(cart_rom(0x10, 0x03));
        mmu.ram_enabled = true;
        mmu.write_byte(0xA000, 0xA5);
        mmu.rtc_registers = [12, 34, 5, 7, 0x40];
        mmu.save_ram(&path).expect("RTC save succeeds");

        let mut loaded = Mmu::new();
        loaded.load_cartridge(cart_rom(0x10, 0x03));
        assert!(loaded.load_ram(&path).expect("RTC load succeeds"));
        assert_eq!(loaded.export_ram()[0], 0xA5);
        assert_eq!(loaded.rtc_state(), [12, 34, 5, 7, 0x40]);
        assert!(std::fs::read(&path).unwrap().starts_with(b"FOXRTC1\0"));
        std::fs::remove_file(&path).ok();
    }
}
