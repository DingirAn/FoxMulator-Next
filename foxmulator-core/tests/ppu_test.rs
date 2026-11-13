//! PPU timing, register and rendering tests derived from Pan Docs.

#[cfg(test)]
mod ppu_tests {
    use foxmulator_core::mmu::{
        Mmu, BGP_ADDR, IF_ADDR, LCDC_ADDR, LYC_ADDR, LY_ADDR, OBP0_ADDR, SCX_ADDR, STAT_ADDR,
        WX_ADDR, WY_ADDR,
    };
    use foxmulator_core::ppu::{Ppu, DMG_COLORS, SCANLINE_CYCLES, VISIBLE_LINES};
    use foxmulator_core::Pixel;

    fn enabled_ppu(mut mmu: Mmu) -> (Ppu, Mmu) {
        mmu.write_byte(LCDC_ADDR, 0x91); // LCD on, BG on, tile data 0x8000, BG map 0x9800
        let mut ppu = Ppu::new();
        ppu.reset(&mut mmu);
        (ppu, mmu)
    }

    #[test]
    fn ly_advances_once_per_scanline() {
        let (mut ppu, mut mmu) = enabled_ppu(Mmu::new());
        assert_eq!(mmu.read_byte(LY_ADDR), 0);

        ppu.tick(SCANLINE_CYCLES - 1, &mut mmu);
        assert_eq!(mmu.read_byte(LY_ADDR), 0, "LY increments after 456 cycles");

        ppu.tick(1, &mut mmu);
        assert_eq!(mmu.read_byte(LY_ADDR), 1);
    }

    #[test]
    fn vblank_starts_at_line_144_and_raises_interrupt() {
        let (mut ppu, mut mmu) = enabled_ppu(Mmu::new());
        mmu.write_byte(IF_ADDR, 0);

        // Advance to LY = 144.
        ppu.tick(SCANLINE_CYCLES * VISIBLE_LINES as u32, &mut mmu);

        assert_eq!(mmu.read_byte(LY_ADDR), VISIBLE_LINES);
        assert_eq!(ppu.mode, 1, "VBlank mode");
        assert_eq!(mmu.read_byte(IF_ADDR) & 0x01, 0x01, "VBlank IF bit");
    }

    #[test]
    fn frame_wraps_ly_to_zero_after_153() {
        let (mut ppu, mut mmu) = enabled_ppu(Mmu::new());
        // 154 scanlines bring LY from 0 back to 0.
        ppu.tick(SCANLINE_CYCLES * 154, &mut mmu);
        assert_eq!(mmu.read_byte(LY_ADDR), 0);
        assert_eq!(ppu.mode, 2, "back to OAM search");
    }

    #[test]
    fn mode_progression_within_scanline() {
        let (mut ppu, mut mmu) = enabled_ppu(Mmu::new());
        assert_eq!(ppu.mode, 2);

        ppu.tick(80, &mut mmu);
        assert_eq!(ppu.mode, 3, "pixel transfer starts at cycle 80");

        ppu.tick(172, &mut mmu);
        assert_eq!(ppu.mode, 0, "HBlank starts at cycle 252");
    }

    #[test]
    fn stat_mode_0_interrupt_on_rising_edge() {
        let (mut ppu, mut mmu) = enabled_ppu(Mmu::new());
        let stat = mmu.read_byte(STAT_ADDR);
        mmu.write_byte(STAT_ADDR, stat | 0x08); // HBlank STAT interrupt
        mmu.write_byte(IF_ADDR, 0);

        ppu.tick(252, &mut mmu);

        assert_eq!(mmu.read_byte(IF_ADDR) & 0x02, 0x02, "STAT IF bit");
    }

    #[test]
    fn lyc_match_sets_stat_bit_and_interrupt() {
        let (mut ppu, mut mmu) = enabled_ppu(Mmu::new());
        mmu.write_byte(LYC_ADDR, 2);
        let stat = mmu.read_byte(STAT_ADDR);
        mmu.write_byte(STAT_ADDR, stat | 0x40); // LYC STAT interrupt
        mmu.write_byte(IF_ADDR, 0);

        ppu.tick(SCANLINE_CYCLES * 2, &mut mmu);

        assert_eq!(mmu.read_byte(LY_ADDR), 2);
        assert_eq!(
            mmu.read_byte(STAT_ADDR) & 0x04,
            0x04,
            "LYC coincidence flag"
        );
        assert_eq!(mmu.read_byte(IF_ADDR) & 0x02, 0x02, "STAT IF bit");
    }

    #[test]
    fn lcd_disabled_holds_ly_at_zero() {
        let mut mmu = Mmu::new();
        mmu.write_byte(LCDC_ADDR, 0x00);
        mmu.write_byte(LY_ADDR, 77);
        let mut ppu = Ppu::new();
        ppu.tick(SCANLINE_CYCLES * 3, &mut mmu);

        assert_eq!(mmu.read_byte(LY_ADDR), 0);
        assert_eq!(ppu.mode, 0);
        assert_eq!(mmu.read_byte(STAT_ADDR) & 0x03, 0);
    }

    #[test]
    fn background_tile_renders_with_bgp() {
        let (mut ppu, mut mmu) = enabled_ppu(Mmu::new());
        mmu.write_byte(BGP_ADDR, 0xE4); // identity mapping

        // Tile 0 row 0: bitplane 1 = 0xFF, bitplane 2 = 0x00 -> colour 1.
        mmu.write_byte(0x8000, 0xFF);
        mmu.write_byte(0x8001, 0x00);
        // BG map entry 0 selects tile 0.
        mmu.write_byte(0x9800, 0x00);

        ppu.render_line(0, &mut mmu);

        assert_eq!(ppu.buffer[0], Pixel(DMG_COLORS[1]));
        assert_eq!(ppu.buffer[159], Pixel(DMG_COLORS[1]));
    }

    #[test]
    fn cgb_background_uses_attribute_bank_and_palette() {
        let mut mmu = Mmu::new();
        mmu.cgb = true;
        let (mut ppu, mut mmu) = enabled_ppu(mmu);

        // Tile 0 is stored in VRAM bank 1 and produces color number 1.
        mmu.write_byte(0xFF4F, 1);
        mmu.write_byte(0x8000, 0xFF);
        mmu.write_byte(0x8001, 0x00);
        // Attribute bank selects CGB palette 1 and tile-data bank 1.
        mmu.write_byte(0x8000 + 0x1800, 0x09);
        mmu.write_byte(0xFF4F, 0);
        mmu.write_byte(0x9800, 0);
        // Palette 1, color 1 = RGB555 red.
        mmu.write_byte(0xFF68, 0x80 | 10);
        mmu.write_byte(0xFF69, 0x1F);
        mmu.write_byte(0xFF69, 0x00);

        ppu.render_line(0, &mut mmu);

        assert_eq!(ppu.buffer[0], Pixel(0xFFFF0000));
    }

    #[test]
    fn cgb_background_signed_tile_mode_uses_9000_block() {
        let mut mmu = Mmu::new();
        mmu.cgb = true;
        let (mut ppu, mut mmu) = enabled_ppu(mmu);
        mmu.write_byte(LCDC_ADDR, 0x81); // CGB, BG on, signed tile mode.

        // Tile 0 differs between the unsigned 8000 block and signed 9000
        // block. CGB does not discard LCDC.4 for BG/window addressing.
        mmu.write_byte(0x8000, 0xFF);
        mmu.write_byte(0x8001, 0x00); // color 1 at 8000
        mmu.write_byte(0x9000, 0x00);
        mmu.write_byte(0x9001, 0xFF); // color 2 at 9000
        mmu.write_byte(0x9800, 0x00);

        // Palette 0 color 1 = red, color 2 = blue.
        mmu.write_byte(0xFF68, 0x82);
        mmu.write_byte(0xFF69, 0x1F);
        mmu.write_byte(0xFF69, 0x00);
        mmu.write_byte(0xFF68, 0x84);
        mmu.write_byte(0xFF69, 0x00);
        mmu.write_byte(0xFF69, 0x7C);

        ppu.render_line(0, &mut mmu);
        assert_eq!(ppu.buffer[0], Pixel(0xFF0000FF));
    }

    #[test]
    fn cgb_background_tile_number_comes_from_bank_zero_with_vbk_one() {
        let mut mmu = Mmu::new();
        mmu.cgb = true;
        let (mut ppu, mut mmu) = enabled_ppu(mmu);

        // Bank 0 contains tile 0 (colour 1) and tile 1 (colour 2), with the
        // map selecting tile 0. Bank 1 contains only the tile attributes.
        mmu.write_byte(0xFF4F, 0);
        mmu.write_byte(0x8000, 0xFF);
        mmu.write_byte(0x8001, 0x00); // tile 0: colour 1
        mmu.write_byte(0x8010, 0x00);
        mmu.write_byte(0x8011, 0xFF); // tile 1: colour 2
        mmu.write_byte(0x9800, 0x00); // bank-0 map: tile 0
        mmu.write_byte(0xFF4F, 1);
        mmu.write_byte(0x9800, 0x01); // bank-1 attribute: palette 1

        // Palette 1: colour 1 = green, colour 2 = blue.
        mmu.write_byte(0xFF68, 0x8A); // palette 1, colour 1
        mmu.write_byte(0xFF69, 0xE0);
        mmu.write_byte(0xFF69, 0x03);
        mmu.write_byte(0xFF69, 0x00);
        mmu.write_byte(0xFF69, 0x7C);

        // Leave VBK=1 to prove PPU map reads do not follow the CPU bank.
        ppu.render_line(0, &mut mmu);

        assert_eq!(ppu.buffer[0], Pixel(0xFF00FF00));
    }

    #[test]
    fn cgb_window_uses_attribute_bank_and_palette() {
        let mut mmu = Mmu::new();
        mmu.cgb = true;
        let (mut ppu, mut mmu) = enabled_ppu(mmu);
        mmu.write_byte(WY_ADDR, 0);
        mmu.write_byte(WX_ADDR, 7); // window starts at screen x=0
        let lcdc = mmu.read_byte(LCDC_ADDR);
        mmu.write_byte(LCDC_ADDR, lcdc | 0x20); // enable window

        mmu.write_byte(0xFF4F, 1);
        mmu.write_byte(0x8000, 0xFF);
        mmu.write_byte(0x8001, 0x00); // color number 1
        mmu.write_byte(0x8000 + 0x1800, 0x09); // bank 1, palette 1
        mmu.write_byte(0xFF4F, 0);
        mmu.write_byte(0x9800, 0);

        // Background palette 1, color 1 = RGB555 blue.
        mmu.write_byte(0xFF68, 0x80 | 10);
        mmu.write_byte(0xFF69, 0x00);
        mmu.write_byte(0xFF69, 0x7C);

        ppu.render_line(0, &mut mmu);

        assert_eq!(ppu.buffer[0], Pixel(0xFF0000FF));
        assert_eq!(ppu.window_line, 1);
    }

    #[test]
    fn cgb_sprite_uses_attribute_bank_and_object_palette() {
        let mut mmu = Mmu::new();
        mmu.cgb = true;
        let (mut ppu, mut mmu) = enabled_ppu(mmu);
        let lcdc = mmu.read_byte(LCDC_ADDR);
        mmu.write_byte(LCDC_ADDR, lcdc | 0x02); // enable OBJ

        // Sprite 0 at screen (0, 0), using tile 1 from VRAM bank 1.
        mmu.write_byte(0xFE00, 16);
        mmu.write_byte(0xFE01, 8);
        mmu.write_byte(0xFE02, 1);
        mmu.write_byte(0xFE03, 0x0A); // VRAM bank 1, CGB object palette 2

        mmu.write_byte(0xFF4F, 1);
        mmu.write_byte(0x8010, 0xFF);
        mmu.write_byte(0x8011, 0x00); // color number 1
        mmu.write_byte(0xFF4F, 0);

        // Object palette 2, color 1 = RGB555 red (offset 2*8 + 1*2).
        mmu.write_byte(0xFF6A, 0x80 | 18);
        mmu.write_byte(0xFF6B, 0x1F);
        mmu.write_byte(0xFF6B, 0x00);

        ppu.render_line(0, &mut mmu);

        assert_eq!(ppu.buffer[0], Pixel(0xFFFF0000));
    }

    #[test]
    fn cgb_sprite_priority_uses_oam_order_over_x_coordinate() {
        let mut mmu = Mmu::new();
        mmu.cgb = true;
        let (mut ppu, mut mmu) = enabled_ppu(mmu);
        let lcdc = mmu.read_byte(LCDC_ADDR);
        mmu.write_byte(LCDC_ADDR, lcdc | 0x02); // enable OBJ

        // Both sprites cover screen x=4. Sprite 0 is farther right, but its
        // lower OAM index wins on CGB. Their tile rows use color 1 and 2.
        mmu.write_byte(0xFE00, 16);
        mmu.write_byte(0xFE01, 12);
        mmu.write_byte(0xFE02, 1);
        mmu.write_byte(0xFE04, 16);
        mmu.write_byte(0xFE05, 8);
        mmu.write_byte(0xFE06, 2);
        mmu.write_byte(0x8010, 0xFF);
        mmu.write_byte(0x8011, 0x00); // tile 1: color 1
        mmu.write_byte(0x8020, 0x00);
        mmu.write_byte(0x8021, 0xFF); // tile 2: color 2

        // Object palette 0: color 1 = red, color 2 = green.
        mmu.write_byte(0xFF6A, 0x80 | 2);
        mmu.write_byte(0xFF6B, 0x1F);
        mmu.write_byte(0xFF6B, 0x00);
        mmu.write_byte(0xFF6B, 0xE0);
        mmu.write_byte(0xFF6B, 0x03);

        ppu.render_line(0, &mut mmu);

        assert_eq!(ppu.buffer[4], Pixel(0xFFFF0000));

        mmu.write_byte(0xFF6C, 1); // CGB OPRI: select X-coordinate priority.
        ppu.render_line(0, &mut mmu);
        assert_eq!(ppu.buffer[4], Pixel(0xFF00FF00));
    }

    #[test]
    fn scx_scrolls_background() {
        let (mut ppu, mut mmu) = enabled_ppu(Mmu::new());
        mmu.write_byte(BGP_ADDR, 0xE4);
        mmu.write_byte(SCX_ADDR, 8); // scroll by one tile to the right

        // Tile 0 is colour 1, tile 1 is colour 2.
        mmu.write_byte(0x8000, 0xFF);
        mmu.write_byte(0x8001, 0x00);
        mmu.write_byte(0x8010, 0x00);
        mmu.write_byte(0x8011, 0xFF);
        mmu.write_byte(0x9800, 0x00);
        mmu.write_byte(0x9801, 0x01);

        ppu.render_line(0, &mut mmu);

        assert_eq!(
            ppu.buffer[0],
            Pixel(DMG_COLORS[2]),
            "first pixel comes from tile 1 (colour index 2) after 8px scroll"
        );
    }

    #[test]
    fn window_renders_and_increments_its_line_counter() {
        let (mut ppu, mut mmu) = enabled_ppu(Mmu::new());
        mmu.write_byte(BGP_ADDR, 0xE4);
        mmu.write_byte(WY_ADDR, 0);
        mmu.write_byte(WX_ADDR, 7); // window starts at x = 0
        let lcdc = mmu.read_byte(LCDC_ADDR);
        mmu.write_byte(LCDC_ADDR, lcdc | 0x20);

        // Window tile 1 row 0: colour 3.
        mmu.write_byte(0x8010, 0xFF);
        mmu.write_byte(0x8011, 0xFF);
        mmu.write_byte(0x9C00, 0x01); // window map at 0x9C00 when LCDC bit 6 set? no: 0x9800

        // LCDC bit 6 is clear, so the window map is 0x9800; write there too.
        mmu.write_byte(0x9800, 0x01);

        ppu.render_line(0, &mut mmu);

        assert_eq!(ppu.buffer[0], Pixel(DMG_COLORS[3]));
        assert_eq!(ppu.window_line, 1);
    }

    #[test]
    fn window_enabled_during_vblank_does_not_cover_next_frame_from_line_zero() {
        let (mut ppu, mut mmu) = enabled_ppu(Mmu::new());
        mmu.write_byte(BGP_ADDR, 0xE4);
        mmu.write_byte(0x8010, 0xFF);
        mmu.write_byte(0x8011, 0xFF);
        mmu.write_byte(0x9800, 0x01); // BG uses a non-zero tile.
        mmu.write_byte(0x9C00, 0x00); // Window uses a blank tile.

        ppu.tick(SCANLINE_CYCLES * 153, &mut mmu);
        assert_eq!(mmu.read_byte(LY_ADDR), 153);
        mmu.write_byte(LCDC_ADDR, 0xF1); // Enable window during VBlank.
        ppu.tick(1, &mut mmu); // Observe the LCDC transition.
        ppu.tick(SCANLINE_CYCLES + 80, &mut mmu);

        assert_eq!(mmu.read_byte(LY_ADDR), 0);
        assert_eq!(ppu.buffer[0], Pixel(DMG_COLORS[3]));
        assert_eq!(ppu.window_line, 0);
    }

    #[test]
    fn vblank_window_transition_preserves_previous_cgb_obj_enable_state() {
        let mut cgb = Mmu::new();
        cgb.cgb = true;
        let (mut ppu, mut mmu) = enabled_ppu(cgb);
        mmu.write_byte(LCDC_ADDR, 0x93); // LCD on, BG and 8x8 OBJ enabled.
        ppu.reset(&mut mmu);
        mmu.write_byte(0xFF6A, 0x82); // OBJ palette 0, colour 1.
        mmu.write_byte(0xFF6B, 0x1F);
        mmu.write_byte(0xFF6B, 0x00);
        mmu.write_byte(0x8012, 0xFF); // OBJ tile 1: colour 1 across row 1.
        mmu.write_byte(0x8013, 0x00);
        mmu.write_byte(0xFE00, 0x58); // screen y=72.
        mmu.write_byte(0xFE01, 0x2F); // screen x=39.
        mmu.write_byte(0xFE02, 0x01);
        assert_eq!(mmu.cgb_palette_word(true, 0, 1), 0x001F);

        ppu.tick(SCANLINE_CYCLES * 153, &mut mmu);
        mmu.write_byte(LCDC_ADDR, 0xF1); // Window on, OBJ bit cleared in VBlank.
        ppu.tick(1, &mut mmu);
        ppu.render_line(73, &mut mmu);

        assert_eq!(ppu.buffer[73 * 160 + 39], Pixel(0xFFFF0000));
    }

    #[test]
    fn sprite_renders_over_background() {
        let (mut ppu, mut mmu) = enabled_ppu(Mmu::new());
        mmu.write_byte(BGP_ADDR, 0xE4);
        mmu.write_byte(OBP0_ADDR, 0xE4);
        let lcdc = mmu.read_byte(LCDC_ADDR);
        mmu.write_byte(LCDC_ADDR, lcdc | 0x02); // enable OBJ

        // Sprite 0 at y=16 (screen y=0), x=8 (screen x=0), tile 1.
        mmu.write_byte(0xFE00, 16);
        mmu.write_byte(0xFE01, 8);
        mmu.write_byte(0xFE02, 1);
        mmu.write_byte(0xFE03, 0);
        // Tile 1 row 0: both bitplanes set -> colour 3.
        mmu.write_byte(0x8010, 0xFF);
        mmu.write_byte(0x8011, 0xFF);

        ppu.render_line(0, &mut mmu);

        assert_eq!(ppu.buffer[0], Pixel(DMG_COLORS[3]));
    }

    #[test]
    fn sprite_behind_background_is_hidden_when_attribute_bit7_set() {
        let (mut ppu, mut mmu) = enabled_ppu(Mmu::new());
        mmu.write_byte(BGP_ADDR, 0xE4);
        mmu.write_byte(OBP0_ADDR, 0xE4);
        let lcdc = mmu.read_byte(LCDC_ADDR);
        mmu.write_byte(LCDC_ADDR, lcdc | 0x02); // enable OBJ

        // Background pixel 0 is colour 1 (non-zero).
        mmu.write_byte(0x8000, 0xFF);
        mmu.write_byte(0x8001, 0x00);
        mmu.write_byte(0x9800, 0x00);

        // Sprite 0 behind BG (attribute bit 7).
        mmu.write_byte(0xFE00, 16);
        mmu.write_byte(0xFE01, 8);
        mmu.write_byte(0xFE02, 1);
        mmu.write_byte(0xFE03, 0x80);
        mmu.write_byte(0x8010, 0xFF);
        mmu.write_byte(0x8011, 0xFF);

        ppu.render_line(0, &mut mmu);

        assert_eq!(
            ppu.buffer[0],
            Pixel(DMG_COLORS[1]),
            "background wins over hidden sprite"
        );
    }

    #[test]
    fn cgb_background_priority_respects_color_zero_semantics() {
        let mut mmu = Mmu::new();
        mmu.cgb = true;
        let (mut ppu, mut mmu) = enabled_ppu(mmu);
        let lcdc = mmu.read_byte(LCDC_ADDR);
        mmu.write_byte(LCDC_ADDR, lcdc | 0x02); // enable OBJ

        // Tile 0 has a CGB map attribute that sets
        // background-over-OBJ priority.
        mmu.write_byte(0xFF4F, 1);
        mmu.write_byte(0x9800, 0x80);
        assert_eq!(mmu.read_vram_bank(1, 0x9800), 0x80);
        mmu.write_byte(0xFF4F, 0);

        // Sprite 0 has a visible colour-1 pixel at screen x=0, y=0.
        mmu.write_byte(0xFE00, 16);
        mmu.write_byte(0xFE01, 8);
        mmu.write_byte(0xFE02, 1);
        mmu.write_byte(0x8010, 0xFF);
        mmu.write_byte(0x8011, 0x00);

        // Object palette 0, colour 1 = red, so a visible sprite is distinct.
        mmu.write_byte(0xFF6A, 0x82);
        mmu.write_byte(0xFF6B, 0x1F);
        mmu.write_byte(0xFF6B, 0x00);

        // Background colour 0 remains transparent to OBJ priority.
        ppu.render_line(0, &mut mmu);
        assert_eq!(ppu.buffer[0], Pixel(0xFFFF0000));

        // A non-zero background colour makes the same priority attribute win.
        mmu.write_byte(0x8000, 0xFF);
        mmu.write_byte(0x8001, 0x00);
        ppu.render_line(0, &mut mmu);
        assert_eq!(ppu.buffer[0], Pixel(0xFF000000));
    }

    #[test]
    fn cgb_master_priority_off_keeps_bg_visible_but_places_obj_above_it() {
        let mut mmu = Mmu::new();
        mmu.cgb = true;
        let (mut ppu, mut mmu) = enabled_ppu(mmu);
        let lcdc = mmu.read_byte(LCDC_ADDR);
        mmu.write_byte(LCDC_ADDR, (lcdc | 0x02) & !0x01);

        // Background tile: colour 1 across the line, with a red CGB palette.
        mmu.write_byte(0x8000, 0xFF);
        mmu.write_byte(0x8001, 0x00);
        mmu.write_byte(0xFF68, 0x82);
        mmu.write_byte(0xFF69, 0x1F);
        mmu.write_byte(0xFF69, 0x00);

        // Object tile: colour 1 at screen x=0, with a blue CGB palette.
        mmu.write_byte(0x8010, 0xFF);
        mmu.write_byte(0x8011, 0x00);
        mmu.write_byte(0xFE00, 16);
        mmu.write_byte(0xFE01, 8);
        mmu.write_byte(0xFE02, 1);
        mmu.write_byte(0xFF6A, 0x82);
        mmu.write_byte(0xFF6B, 0x00);
        mmu.write_byte(0xFF6B, 0x7C);

        ppu.render_line(0, &mut mmu);

        assert_eq!(ppu.buffer[0], Pixel(0xFF0000FF));
        assert_eq!(ppu.buffer[20], Pixel(0xFFFF0000));
    }

    #[test]
    fn sprite_flip_horizontal_mirrors_tile() {
        let (mut ppu, mut mmu) = enabled_ppu(Mmu::new());
        mmu.write_byte(BGP_ADDR, 0xE4);
        mmu.write_byte(OBP0_ADDR, 0xE4);
        let lcdc = mmu.read_byte(LCDC_ADDR);
        mmu.write_byte(LCDC_ADDR, lcdc | 0x02); // enable OBJ

        // Sprite tile 1: only the leftmost pixel is colour 1 in b1.
        mmu.write_byte(0x8010, 0x80);
        mmu.write_byte(0x8011, 0x00);
        // Horizontal flip -> colour appears at the rightmost pixel.
        mmu.write_byte(0xFE00, 16);
        mmu.write_byte(0xFE01, 8);
        mmu.write_byte(0xFE02, 1);
        mmu.write_byte(0xFE03, 0x20);

        ppu.render_line(0, &mut mmu);

        assert_eq!(ppu.buffer[0], Pixel(DMG_COLORS[0]));
        assert_eq!(ppu.buffer[7], Pixel(DMG_COLORS[1]));
    }

    #[test]
    fn ten_sprite_limit_per_line() {
        let (mut ppu, mut mmu) = enabled_ppu(Mmu::new());
        mmu.write_byte(BGP_ADDR, 0xE4);
        mmu.write_byte(OBP0_ADDR, 0xE4);
        let lcdc = mmu.read_byte(LCDC_ADDR);
        mmu.write_byte(LCDC_ADDR, lcdc | 0x02); // enable OBJ

        // Pixel 0 background is colour 0, so every drawn sprite would win.
        // 11 sprites all covering screen x=0..7; only the first 10 count.
        for i in 0..11u16 {
            let oam = 0xFE00 + i * 4;
            mmu.write_byte(oam, 16);
            mmu.write_byte(oam + 1, 8);
            mmu.write_byte(oam + 2, 1 + i as u8);
            mmu.write_byte(oam + 3, 0);
            // Each tile row 0 is colour 1.
            let tile_addr = 0x8000 + (1 + i) * 0x10;
            mmu.write_byte(tile_addr, 0xFF);
            mmu.write_byte(tile_addr + 1, 0x00);
        }

        ppu.render_line(0, &mut mmu);

        // The 11th sprite uses tile 11; it must not be drawn.
        let drawn = ppu.buffer[0];
        assert_ne!(drawn, Pixel(DMG_COLORS[0]), "a sprite was drawn");
        // Sprite 0 wins priority, so its palette colour is the identity
        // mapping's colour 1 (dark-ish shade index 1).
        assert_eq!(drawn, Pixel(DMG_COLORS[1]));
    }
}
