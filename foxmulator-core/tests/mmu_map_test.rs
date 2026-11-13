//! MMU address-map and MBC banking tests derived from Pan Docs.

#[cfg(test)]
mod mmu_map {
    use foxmulator_core::cartridge::MbcType;
    use foxmulator_core::mmu::Mmu;

    fn rom_with_banks(banks: usize, mbc_code: u8, ram_code: u8) -> Vec<u8> {
        let mut rom = vec![0u8; banks * 0x4000];
        for bank in 0..banks {
            for offset in 0..0x4000 {
                rom[bank * 0x4000 + offset] = bank as u8;
            }
        }
        rom[0x0147] = mbc_code;
        rom[0x0148] = match banks {
            2 => 0x00,
            4 => 0x01,
            8 => 0x02,
            16 => 0x03,
            _ => 0x00,
        };
        rom[0x0149] = ram_code;
        rom
    }

    #[test]
    fn echo_ram_unusable_and_hram() {
        let mut mmu = Mmu::new();
        mmu.write_byte(0xC123, 0xAB);
        assert_eq!(mmu.read_byte(0xE123), 0xAB, "echo reads WRAM");
        mmu.write_byte(0xE456, 0xCD);
        assert_eq!(mmu.read_byte(0xC456), 0xCD, "echo writes WRAM");

        mmu.write_byte(0xFEA0, 0x12);
        assert_eq!(mmu.read_byte(0xFEA0), 0xFF, "unusable region reads 0xFF");

        mmu.write_byte(0xFF80, 0x77);
        assert_eq!(mmu.read_byte(0xFF80), 0x77, "HRAM is readable/writable");

        mmu.write_byte(0xFFFF, 0x1F);
        assert_eq!(mmu.read_byte(0xFFFF), 0x1F, "IE register");
    }

    #[test]
    fn cgb_vram_and_wram_banks_are_isolated() {
        let mut mmu = Mmu::new();
        mmu.cgb = true;

        mmu.write_byte(0x8000, 0x10);
        mmu.write_byte(0xFF4F, 0x01);
        mmu.write_byte(0x8000, 0x21);
        assert_eq!(mmu.read_byte(0x8000), 0x21);
        mmu.write_byte(0xFF4F, 0x00);
        assert_eq!(mmu.read_byte(0x8000), 0x10);

        mmu.write_byte(0xD000, 0x31); // bank 1
        mmu.write_byte(0xFF70, 0x02);
        mmu.write_byte(0xD000, 0x42); // bank 2
        assert_eq!(mmu.read_byte(0xD000), 0x42);
        mmu.write_byte(0xFF70, 0x01);
        assert_eq!(mmu.read_byte(0xD000), 0x31);

        // Echo RAM follows the currently selected CGB WRAM bank.
        mmu.write_byte(0xF000, 0x53);
        assert_eq!(mmu.read_byte(0xD000), 0x53);
        assert_eq!(mmu.read_byte(0xFF70), 0xF9);
    }

    #[test]
    fn cgb_key1_reports_speed_and_prepare_bits() {
        let mut mmu = Mmu::new();
        assert_eq!(mmu.read_byte(0xFF4D), 0xFF, "KEY1 is unmapped on DMG");

        mmu.cgb = true;
        assert_eq!(mmu.read_byte(0xFF4D), 0x7E, "normal speed, not armed");
        mmu.write_byte(0xFF4D, 0x01);
        assert_eq!(mmu.read_byte(0xFF4D), 0x7F, "prepare bit is visible");

        mmu.double_speed = true;
        assert_eq!(mmu.read_byte(0xFF4D), 0xFF, "double speed and armed");
        mmu.write_byte(0xFF4D, 0x00);
        assert_eq!(mmu.read_byte(0xFF4D), 0xFE, "double speed, not armed");
    }

    #[test]
    fn cgb_bank_registers_are_unmapped_on_dmg() {
        let mut mmu = Mmu::new();
        mmu.write_byte(0x8000, 0x10);
        mmu.write_byte(0xFF4F, 0x01);
        mmu.write_byte(0x8000, 0x20);
        assert_eq!(mmu.read_byte(0x8000), 0x20);
        assert_eq!(mmu.read_byte(0xFF4F), 0xFF);
        assert_eq!(mmu.read_byte(0xFF70), 0xFF);
        for addr in 0xFF51..=0xFF55 {
            assert_eq!(mmu.read_byte(addr), 0xFF, "DMG HDMA register {addr:04X}");
        }
        assert_eq!(mmu.read_byte(0xFF6C), 0xFF, "DMG OPRI register");

        mmu.cgb = true;
        mmu.write_byte(0xFF6C, 0xFF);
        assert_eq!(mmu.read_byte(0xFF6C), 1, "CGB OPRI masks reserved bits");
    }

    #[test]
    fn cgb_palette_ram_auto_increments_and_roundtrips() {
        let mut mmu = Mmu::new();
        mmu.cgb = true;
        mmu.write_byte(0xFF68, 0xC0 | 2); // bit 6 is reserved and must clear.
        mmu.write_byte(0xFF69, 0x1F);
        mmu.write_byte(0xFF69, 0x00);
        assert_eq!(mmu.read_byte(0xFF68), 0x84);
        mmu.write_byte(0xFF68, 2);
        assert_eq!(mmu.read_byte(0xFF69), 0x1F);
        mmu.write_byte(0xFF6A, 0x80 | 8);
        mmu.write_byte(0xFF6B, 0x00);
        mmu.write_byte(0xFF6B, 0x7C);
        assert_eq!(mmu.cgb_palette_word(true, 1, 0), 0x7C00);

        let mut dmg = Mmu::new();
        dmg.write_byte(0xFF68, 0xC2);
        dmg.write_byte(0xFF69, 0xAA);
        assert_eq!(dmg.read_byte(0xFF68), 0xFF);
        assert_eq!(dmg.read_byte(0xFF69), 0xFF);
    }

    #[test]
    fn oam_dma_copies_progressively_and_blocks_cpu_bus() {
        let mut mmu = Mmu::new();
        for offset in 0..0xA0u16 {
            mmu.write_byte(0xC000 + offset, offset as u8 ^ 0x5A);
        }
        mmu.write_byte(0x0100, 0xA5);
        mmu.write_byte(0xFF80, 0x3C);
        let before = mmu.peripheral_ticks;
        mmu.write_byte(0xFF46, 0xC0);
        assert!(mmu.dma_active);
        assert_eq!(mmu.peripheral_ticks - before, 0);
        assert_eq!(mmu.read_byte(0xFE00), 0);

        mmu.tick(4);
        assert!(mmu.dma_active);
        assert_eq!(mmu.read_byte(0xFE00), 0x5A);
        assert_eq!(mmu.cpu_read_byte(0x0100), 0xFF, "DMA blocks ROM reads");
        assert_eq!(mmu.cpu_read_byte(0xFF80), 0x3C, "HRAM remains accessible");
        mmu.cpu_write_byte(0xC000, 0xEE);
        assert_eq!(mmu.read_byte(0xC000), 0x5A, "DMA blocks non-HRAM writes");

        mmu.tick(636);
        assert!(!mmu.dma_active);
        assert_eq!(mmu.peripheral_ticks - before, 640);
        assert_eq!(mmu.read_byte(0xFE9F), 0x5A ^ 0x9F);
    }

    #[test]
    fn cpu_vram_and_oam_access_follow_ppu_mode() {
        let mut mmu = Mmu::new();
        mmu.write_byte(0x8000, 0x12);
        mmu.write_byte(0xFE00, 0x34);
        mmu.write_byte(0xFF40, 0x80); // LCD enabled; default mode is OAM search.

        assert_eq!(
            mmu.cpu_read_byte(0x8000),
            0x12,
            "VRAM is readable in mode 2"
        );
        assert_eq!(mmu.cpu_read_byte(0xFE00), 0xFF, "OAM is blocked in mode 2");
        mmu.cpu_write_byte(0x8000, 0x56);
        mmu.cpu_write_byte(0xFE00, 0x78);
        assert_eq!(mmu.read_byte(0x8000), 0x56);
        assert_eq!(mmu.read_byte(0xFE00), 0x34);

        mmu.ppu_mode = 3;
        assert_eq!(mmu.cpu_read_byte(0x8000), 0xFF, "VRAM is blocked in mode 3");
        assert_eq!(mmu.cpu_read_byte(0xFE00), 0xFF, "OAM is blocked in mode 3");
        mmu.cpu_write_byte(0x8000, 0x9A);
        mmu.cpu_write_byte(0xFE00, 0xBC);
        assert_eq!(mmu.read_byte(0x8000), 0x56);
        assert_eq!(mmu.read_byte(0xFE00), 0x34);

        mmu.ppu_mode = 0;
        assert_eq!(
            mmu.cpu_read_byte(0x8000),
            0x56,
            "VRAM is available in HBlank"
        );
        assert_eq!(
            mmu.cpu_read_byte(0xFE00),
            0x34,
            "OAM is available in HBlank"
        );
    }

    #[test]
    fn cgb_gdma_copies_blocks_and_reports_completion() {
        let mut mmu = Mmu::new();
        mmu.cgb = true;
        for offset in 0..0x20u16 {
            mmu.write_byte(0xC000 + offset, 0x80 | offset as u8);
        }
        mmu.write_byte(0xFF51, 0xC0);
        mmu.write_byte(0xFF52, 0x00);
        mmu.write_byte(0xFF53, 0x00);
        mmu.write_byte(0xFF54, 0x20);
        let before = mmu.peripheral_ticks;
        mmu.write_byte(0xFF55, 0x01); // General DMA, two 16-byte blocks.
        assert_eq!(mmu.peripheral_ticks - before, 64);
        assert_eq!(mmu.read_byte(0x8020), 0x80);
        assert_eq!(mmu.read_byte(0x803F), 0x9F);
        assert!(!mmu.hdma_active);
        assert_eq!(mmu.read_byte(0xFF55), 0xFF);
    }

    #[test]
    fn cgb_hblank_dma_copies_one_block_per_hblank() {
        let mut mmu = Mmu::new();
        mmu.cgb = true;
        for offset in 0..0x20u16 {
            mmu.write_byte(0xC000 + offset, offset as u8);
        }
        mmu.write_byte(0xFF51, 0xC0);
        mmu.write_byte(0xFF53, 0x00);
        mmu.write_byte(0xFF55, 0x80 | 0x01);
        assert!(mmu.hdma_active);
        mmu.hdma_hblank_transfer();
        assert_eq!(mmu.read_byte(0x8000), 0);
        assert_eq!(mmu.read_byte(0x800F), 0x0F);
        assert_eq!(mmu.read_byte(0xFF55), 0x80);
        mmu.hdma_hblank_transfer();
        assert_eq!(mmu.read_byte(0x8010), 0x10);
        assert!(!mmu.hdma_active);
    }

    #[test]
    fn cgb_hblank_dma_cancellation_reports_remaining_blocks() {
        let mut mmu = Mmu::new();
        mmu.cgb = true;
        for offset in 0..0x30u16 {
            mmu.write_byte(0xC000 + offset, offset as u8);
        }
        mmu.write_byte(0xFF51, 0xC0);
        mmu.write_byte(0xFF53, 0x00);
        mmu.write_byte(0xFF55, 0x80 | 0x02); // three HBlank blocks
        mmu.hdma_hblank_transfer();
        assert_eq!(
            mmu.read_byte(0xFF55),
            0x81,
            "two blocks remain while active"
        );

        mmu.write_byte(0xFF55, 0x00); // cancel HBlank DMA
        assert!(!mmu.hdma_active);
        assert_eq!(
            mmu.read_byte(0xFF55),
            0x01,
            "one block remains after cancel"
        );
    }

    #[test]
    fn oam_read_corruption_is_deterministic_for_a_fixed_ppu_phase() {
        let mut mmu = Mmu::new();
        mmu.write_byte(0xFF40, 0x80);
        for offset in 0..0xA0u16 {
            mmu.write_byte(0xFE00 + offset, offset as u8);
        }
        mmu.advance_ppu_clock(20);

        // The fixed phase selects row 0x28 in the diagnostic clock. Capture
        // the reference words before applying the documented read glitch.
        let row = 0x28usize;
        let a = u16::from_le_bytes([row as u8, (row + 1) as u8]);
        let b = u16::from_le_bytes([(row - 8) as u8, (row - 7) as u8]);
        let c = u16::from_le_bytes([(row - 4) as u8, (row - 3) as u8]);
        let expected = b | (a & c);
        mmu.oam_bug_read_context(Some((0xFEF0, 0xFEF0, "test")));

        assert_eq!(mmu.read_byte(0xFE20), expected as u8);
        assert_eq!(mmu.read_byte(0xFE21), (expected >> 8) as u8);
        assert_eq!(mmu.read_byte(0xFE28), expected as u8);
        assert_eq!(mmu.read_byte(0xFE29), (expected >> 8) as u8);
    }

    #[test]
    fn mbc1_rom_banking_and_ram_enable() {
        let rom = rom_with_banks(4, 0x01, 0x02); // MBC1, 8 KiB RAM
        let mut mmu = Mmu::new();
        mmu.load_cartridge(rom);
        assert_eq!(mmu.cartridge.as_ref().unwrap().mbc, MbcType::Mbc1);

        assert_eq!(mmu.read_byte(0x0000), 0, "fixed bank 0");
        assert_eq!(mmu.read_byte(0x4000), 1, "default switchable bank is 1");

        mmu.write_byte(0x2000, 0x02);
        assert_eq!(mmu.rom_bank(), 2);
        assert_eq!(mmu.read_byte(0x4000), 2);

        // The MBC register must not modulo an absent physical bank back to
        // bank 1. Software can use the resulting open bus as a table
        // sentinel; only the forbidden MBC1 bank number zero is remapped.
        mmu.write_byte(0x2000, 0x04);
        assert_eq!(mmu.rom_bank(), 4);
        assert_eq!(mmu.read_byte(0x4000), 0xFF);

        mmu.write_byte(0x2000, 0x00);
        assert_eq!(mmu.rom_bank(), 1, "bank 0 maps to bank 1");

        // RAM is disabled until 0x0A is written to the enable register.
        assert_eq!(mmu.read_byte(0xA000), 0xFF);
        mmu.write_byte(0xA000, 0x55);
        assert_eq!(mmu.read_byte(0xA000), 0xFF);
        mmu.write_byte(0x0000, 0x0A);
        mmu.write_byte(0xA000, 0x55);
        assert_eq!(mmu.read_byte(0xA000), 0x55);
        mmu.write_byte(0x0000, 0x00);
        assert_eq!(mmu.read_byte(0xA000), 0xFF, "RAM disable");
    }

    #[test]
    fn mbc1_ram_banking_in_mode_1() {
        let rom = rom_with_banks(4, 0x01, 0x03); // 32 KiB RAM
        let mut mmu = Mmu::new();
        mmu.load_cartridge(rom);
        mmu.write_byte(0x0000, 0x0A);
        mmu.write_byte(0x6000, 0x01); // RAM banking mode
        mmu.write_byte(0x4000, 0x01); // RAM bank 1
        mmu.write_byte(0xA000, 0x11);
        mmu.write_byte(0x4000, 0x02); // RAM bank 2
        assert_eq!(mmu.read_byte(0xA000), 0x00, "bank 2 is zeroed");
        mmu.write_byte(0xA000, 0x22);
        mmu.write_byte(0x4000, 0x01);
        assert_eq!(mmu.read_byte(0xA000), 0x11, "bank 1 retains data");
    }

    #[test]
    fn mbc1_mode_zero_keeps_ram_on_bank_zero() {
        let rom = rom_with_banks(4, 0x01, 0x03); // MBC1, 32 KiB RAM
        let mut mmu = Mmu::new();
        mmu.load_cartridge(rom);
        mmu.write_byte(0x0000, 0x0A);
        mmu.write_byte(0x6000, 0x01); // expose RAM banks
        mmu.write_byte(0x4000, 0x01);
        mmu.write_byte(0xA000, 0x11);
        mmu.write_byte(0x6000, 0x00); // return to ROM banking mode
        mmu.write_byte(0x4000, 0x02); // selects ROM upper bits, not RAM bank 2
        assert_eq!(mmu.read_byte(0xA000), 0x00, "mode 0 must expose RAM bank 0");
        mmu.write_byte(0xA000, 0x22);
        mmu.write_byte(0x6000, 0x01);
        mmu.write_byte(0x4000, 0x01);
        assert_eq!(mmu.read_byte(0xA000), 0x11, "RAM bank 1 remains separate");
    }

    #[test]
    fn mbc1_mode_one_maps_upper_bits_to_fixed_rom_window() {
        let rom = rom_with_banks(64, 0x01, 0x03);
        let mut mmu = Mmu::new();
        mmu.load_cartridge(rom);
        mmu.write_byte(0x6000, 0x01); // RAM-banking mode
        mmu.write_byte(0x4000, 0x01); // fixed window bank 0x20
        mmu.write_byte(0x2000, 0x03); // switchable window bank 3
        assert_eq!(mmu.read_byte(0x0000), 0x20);
        assert_eq!(mmu.read_byte(0x4000), 0x03);
    }

    #[test]
    fn mbc2_rom_bank_and_4bit_ram() {
        let rom = rom_with_banks(4, 0x05, 0x00);
        let mut mmu = Mmu::new();
        mmu.load_cartridge(rom);

        // Address bit 8 set selects the ROM bank on MBC2.
        mmu.write_byte(0x2100, 0x02);
        assert_eq!(mmu.rom_bank(), 2);
        assert_eq!(mmu.read_byte(0x4000), 2);

        mmu.write_byte(0x0000, 0x0A); // RAM enable (bit 8 clear)
        mmu.write_byte(0xA000, 0xAB);
        assert_eq!(
            mmu.read_byte(0xA000),
            0xFB,
            "MBC2 stores 4 bits, high nibble 0xF"
        );
    }

    #[test]
    fn mbc3_ram_and_rtc_registers() {
        let rom = rom_with_banks(4, 0x0F, 0x02);
        let mut mmu = Mmu::new();
        mmu.load_cartridge(rom);
        mmu.write_byte(0x0000, 0x0A);

        mmu.write_byte(0x2000, 0x02);
        assert_eq!(mmu.read_byte(0x4000), 2);

        // RAM bank 0.
        mmu.write_byte(0x4000, 0x00);
        mmu.write_byte(0xA000, 0x33);
        assert_eq!(mmu.read_byte(0xA000), 0x33);

        // RTC seconds register at select 0x08.
        mmu.write_byte(0x4000, 0x08);
        mmu.write_byte(0xA000, 0x2A); // 42 seconds; upper bits are ignored.
        mmu.write_byte(0x6000, 0x00);
        mmu.write_byte(0x6000, 0x01); // latch
        assert_eq!(mmu.read_byte(0xA000), 0x2A);

        // Back to RAM bank 0, old value still present.
        mmu.write_byte(0x4000, 0x00);
        assert_eq!(mmu.read_byte(0xA000), 0x33);
    }

    #[test]
    fn mbc5_rom_and_ram_banking() {
        let rom = rom_with_banks(8, 0x19, 0x03); // 32 KiB RAM
        let mut mmu = Mmu::new();
        mmu.load_cartridge(rom);

        mmu.write_byte(0x2000, 0x03); // low 8 bits of bank
        assert_eq!(mmu.rom_bank(), 3);
        assert_eq!(mmu.read_byte(0x4000), 3);

        mmu.write_byte(0x3000, 0x01); // bit 8 -> bank 0x103, wrapped modulo 8
        assert_eq!(mmu.read_byte(0x4000), (mmu.rom_bank() % 8) as u8);

        mmu.write_byte(0x0000, 0x0A);
        mmu.write_byte(0x4000, 0x02); // RAM bank 2
        mmu.write_byte(0xA000, 0x99);
        mmu.write_byte(0x4000, 0x03);
        assert_eq!(mmu.read_byte(0xA000), 0x00);
        mmu.write_byte(0x4000, 0x02);
        assert_eq!(mmu.read_byte(0xA000), 0x99);
    }
}
