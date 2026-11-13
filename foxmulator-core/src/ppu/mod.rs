//! Pixel Processing Unit (PPU) for Game Boy / Game Boy Color.
//!
//! Implements the DMG scanline timing, LCDC/STAT/LY/LYC registers, background
//! and window tile rendering, and sprite compositing. Palette, VRAM and OAM
//! storage remain in the MMU so the CPU sees the same memory the PPU renders.
//!
//! Reference: Pan Docs, "Rendering" and "LCD Registers"; the original
//! FoxMulator JavaScript implementation in `FoxMulator_1.0.1_Link_Dual.html`.
//!
//! Timing: each scanline is 456 T-cycles. Modes within a visible line are
//! OAM search (0-80), pixel transfer (80-252) and HBlank (252-456).
//! Lines 144-153 are VBlank. A frame is 154 lines = 70224 T-cycles.
//!
//! CGB palette RAM, VRAM bank selection and CGB tile attributes are supported.
//! Known hardware-accuracy gaps include OAM DMA contention/timing and the STAT
//! write quirk; these are tracked separately from the CGB rendering path.

use crate::mmu::{
    Mmu, BGP_ADDR, LCDC_ADDR, LYC_ADDR, LY_ADDR, OBP0_ADDR, OBP1_ADDR, SCX_ADDR, SCY_ADDR,
    STAT_ADDR, WX_ADDR, WY_ADDR,
};
use crate::{Pixel, PIXEL_BUFFER_SIZE};

/// Visible scanlines per frame.
pub const VISIBLE_LINES: u8 = 144;
/// Scanlines per frame including VBlank.
pub const TOTAL_LINES: u8 = 154;
/// T-cycles per scanline.
pub const SCANLINE_CYCLES: u32 = 456;

fn default_bg_colors() -> [u8; 160] {
    [0; 160]
}

fn default_bg_priority() -> [bool; 160] {
    [false; 160]
}

/// DMG shades, index 0 (lightest) to 3 (darkest), in 0xAARRGGBB.
pub const DMG_COLORS: [u32; 4] = [0xFFE0F8D0, 0xFF88C070, 0xFF346856, 0xFF081820];

/// Run one scanline worth of PPU timing.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Ppu {
    /// Framebuffer holding pixels in RGBA8888 format (0xAARRGGBB).
    pub buffer: Vec<Pixel>,
    /// Current STAT mode: 0 = HBlank, 1 = VBlank, 2 = OAM search, 3 = transfer.
    pub mode: u8,
    /// T-cycles elapsed in the current scanline (mirrored from the MMU clock).
    pub cycles: u32,
    /// Window internal line counter, reset at the start of each frame.
    pub window_line: u8,
    /// Previous state of the STAT interrupt line (edge detection).
    stat_line: bool,
    /// Set when the PPU wraps from line 153 back to line 0.
    pub frame_ready: bool,
    /// Last MMU peripheral clock observed. A zero delta identifies the
    /// standalone `Ppu::tick` test path; System-driven ticks use the MMU's
    /// already-positioned clock so LCD writes do not count time while off.
    last_peripheral_ticks: u64,
    /// The first LCD-on line exposes LY=1 four T-cycles before its internal
    /// 452-T OAM-bug phase boundary.
    first_line_ly_advanced: bool,
    /// Whether LCDC currently has the window enabled.
    window_enabled: bool,
    /// mGBA/hardware-compatible state for a window enabled after the visible
    /// area: it must not suddenly cover line zero of the next frame when the
    /// window was opened during VBlank.
    window_blocked: bool,
    /// OBJ enable state from before a VBlank window transition. 1942 changes
    /// LCDC from D3 to F1 in VBlank; mGBA keeps the previous OBJ state until
    /// the deferred window transition becomes visible.
    window_obj_enabled: bool,
    /// Background colour index per pixel (used for DMG sprite priority).
    #[serde(skip, default = "default_bg_colors")]
    bg_color_num: [u8; 160],
    /// Background-to-OAM priority bit per pixel (CGB attribute bit 7).
    #[serde(skip, default = "default_bg_priority")]
    bg_priority: [bool; 160],
    /// Pixels already claimed by a higher-priority sprite on this line.
    #[serde(skip, default = "default_bg_priority")]
    sprite_drawn: [bool; 160],
}

impl Default for Ppu {
    fn default() -> Self {
        Self {
            buffer: vec![Pixel::default(); PIXEL_BUFFER_SIZE],
            mode: 2,
            cycles: 0,
            window_line: 0,
            stat_line: false,
            frame_ready: false,
            last_peripheral_ticks: 0,
            first_line_ly_advanced: false,
            window_enabled: false,
            window_blocked: false,
            window_obj_enabled: true,
            bg_color_num: [0; 160],
            bg_priority: [false; 160],
            sprite_drawn: [false; 160],
        }
    }
}

impl Ppu {
    pub fn new() -> Self {
        Self::default()
    }

    /// Map a DMG palette register value and colour index to a framebuffer pixel.
    pub fn dmg_pixel(palette: u8, color_num: u8) -> Pixel {
        let shade = (palette >> (color_num * 2)) & 0x03;
        Pixel(DMG_COLORS[shade as usize])
    }

    fn cgb_pixel(mmu: &Mmu, object: bool, palette: u8, color_num: u8) -> Pixel {
        let rgb555 = mmu.cgb_palette_word(object, palette, color_num);
        let red = ((rgb555 & 0x1F) as u32 * 255 + 15) / 31;
        let green = (((rgb555 >> 5) & 0x1F) as u32 * 255 + 15) / 31;
        let blue = (((rgb555 >> 10) & 0x1F) as u32 * 255 + 15) / 31;
        Pixel(0xFF00_0000 | (red << 16) | (green << 8) | blue)
    }

    /// Reset the PPU to the state right after LCD enable and advance LY to 0.
    pub fn reset(&mut self, mmu: &mut Mmu) {
        self.mode = 2;
        self.cycles = 0;
        self.window_line = 0;
        self.stat_line = false;
        mmu.write_byte(LY_ADDR, 0);
        let stat = mmu.read_byte(STAT_ADDR);
        mmu.write_byte(STAT_ADDR, (stat & 0x7C) | self.mode);
        self.update_lyc(mmu);
        mmu.reset_ppu_clock();
        mmu.ppu_mode = self.mode;
        self.last_peripheral_ticks = mmu.peripheral_ticks;
        self.first_line_ly_advanced = false;
        self.window_enabled = (mmu.read_byte(LCDC_ADDR) & 0x20) != 0;
        self.window_blocked = false;
        self.window_obj_enabled = (mmu.read_byte(LCDC_ADDR) & 0x02) != 0;
    }

    /// Advance the PPU by `cycles` T-cycles.
    pub fn tick(&mut self, cycles: u32, mmu: &mut Mmu) {
        let lcdc = mmu.read_byte(LCDC_ADDR);
        let window_enabled = (lcdc & 0x20) != 0;
        if window_enabled != self.window_enabled {
            if window_enabled {
                self.window_blocked = mmu.read_byte(LY_ADDR) >= VISIBLE_LINES;
                if !self.window_blocked {
                    self.window_obj_enabled = (lcdc & 0x02) != 0;
                }
            } else {
                self.window_blocked = false;
                self.window_obj_enabled = (lcdc & 0x02) != 0;
                self.window_line = 0;
            }
            self.window_enabled = window_enabled;
        } else if !self.window_blocked {
            self.window_obj_enabled = (lcdc & 0x02) != 0;
        }
        if (lcdc & 0x80) == 0 {
            // LCD disabled: LY is held at 0 and the PPU stays in mode 0.
            self.cycles = 0;
            self.window_line = 0;
            self.mode = 0;
            mmu.write_byte(LY_ADDR, 0);
            let stat = mmu.read_byte(STAT_ADDR);
            mmu.write_byte(STAT_ADDR, (stat & 0x7C) | self.mode);
            self.update_lyc(mmu);
            mmu.ppu_line_cycles = 0;
            mmu.ppu_mode = 0;
            return;
        }

        if mmu.peripheral_ticks != self.last_peripheral_ticks {
            self.cycles = mmu.ppu_line_cycles;
            self.last_peripheral_ticks = mmu.peripheral_ticks;
        } else {
            self.cycles += cycles;
        }

        if mmu.ppu_first_line && !self.first_line_ly_advanced && self.cycles >= 448 {
            mmu.write_byte(LY_ADDR, 1);
            self.update_lyc(mmu);
            self.first_line_ly_advanced = true;
        }

        while self.cycles >= SCANLINE_CYCLES {
            self.cycles -= SCANLINE_CYCLES;
            mmu.ppu_first_line = false;
            let mut ly = if self.first_line_ly_advanced {
                1
            } else {
                mmu.read_byte(LY_ADDR).wrapping_add(1)
            };
            self.first_line_ly_advanced = false;
            if ly > (TOTAL_LINES - 1) {
                ly = 0;
                self.window_line = 0;
                self.frame_ready = true;
            }
            mmu.write_byte(LY_ADDR, ly);
            self.update_lyc(mmu);

            if ly == VISIBLE_LINES {
                self.set_mode(1, mmu);
                // VBlank interrupt.
                let if_reg = mmu.read_byte(crate::mmu::IF_ADDR);
                mmu.write_byte(crate::mmu::IF_ADDR, if_reg | 0x01);
            } else if ly < VISIBLE_LINES {
                self.set_mode(2, mmu);
            }
        }

        let ly = mmu.read_byte(LY_ADDR);
        if ly < VISIBLE_LINES {
            let new_mode = if self.cycles < 80 {
                2
            } else if self.cycles < 252 {
                3
            } else {
                0
            };
            if new_mode != self.mode {
                let was_drawing = self.mode == 3;
                self.set_mode(new_mode, mmu);
                if new_mode == 3 && !was_drawing {
                    self.render_line(ly, mmu);
                } else if new_mode == 0 {
                    // CGB HBlank DMA occupies the first eight M-cycles of
                    // HBlank and advances the shared machine clock.
                    let hdma_was_active = mmu.hdma_active && mmu.hdma_hblank_mode;
                    mmu.hdma_hblank_transfer();
                    if hdma_was_active {
                        self.cycles += 32;
                    }
                }
            }
        } else if self.mode != 1 {
            self.set_mode(1, mmu);
        }

        // The MMU advances this mirror while the CPU executes, so OAM-bug
        // accesses see the correct intra-instruction position. Standalone PPU
        // tests call this method directly, therefore keep the mirror in sync
        // here as well.
        mmu.ppu_line_cycles = self.cycles;
        mmu.ppu_mode = self.mode;
    }

    fn set_mode(&mut self, mode: u8, mmu: &mut Mmu) {
        if self.mode == mode {
            return;
        }
        self.mode = mode;
        let stat = mmu.read_byte(STAT_ADDR);
        mmu.write_byte(STAT_ADDR, (stat & 0x7C) | mode);
        self.update_stat(mmu);
    }

    fn update_lyc(&mut self, mmu: &mut Mmu) {
        let ly = mmu.read_byte(LY_ADDR);
        let lyc = mmu.read_byte(LYC_ADDR);
        let mut stat = mmu.read_byte(STAT_ADDR);
        if ly == lyc {
            stat |= 0x04;
        } else {
            stat &= !0x04;
        }
        mmu.write_byte(STAT_ADDR, stat);
        self.update_stat(mmu);
    }

    /// Raise the STAT interrupt (IF bit 1) on a rising edge of the STAT line.
    fn update_stat(&mut self, mmu: &mut Mmu) {
        let stat = mmu.read_byte(STAT_ADDR);
        let ly = mmu.read_byte(LY_ADDR);
        let lyc = mmu.read_byte(LYC_ADDR);

        let mut line = false;
        match self.mode {
            0 => line |= (stat & 0x08) != 0,
            1 => line |= (stat & 0x10) != 0,
            2 => line |= (stat & 0x20) != 0,
            _ => {}
        }
        if (stat & 0x40) != 0 && ly == lyc {
            line = true;
        }

        if line && !self.stat_line {
            let if_reg = mmu.read_byte(crate::mmu::IF_ADDR);
            mmu.write_byte(crate::mmu::IF_ADDR, if_reg | 0x02);
        }
        self.stat_line = line;
    }

    /// Render one visible scanline into the framebuffer.
    pub fn render_line(&mut self, ly: u8, mmu: &mut Mmu) {
        if ly >= VISIBLE_LINES {
            return;
        }

        let lcdc = mmu.read_byte(LCDC_ADDR);
        let bgp = mmu.read_byte(BGP_ADDR);
        let base = ly as usize * 160;

        let blank = Self::dmg_pixel(bgp, 0);
        for x in 0..160 {
            self.buffer[base + x] = blank;
            self.bg_color_num[x] = 0;
            self.bg_priority[x] = false;
            self.sprite_drawn[x] = false;
        }

        // On DMG, LCDC bit 0 disables the background and window. On CGB it is
        // the BG/window master-priority switch: the layers remain visible,
        // but OBJ pixels are allowed to appear over them regardless of the
        // tile-map priority attribute. This distinction is exercised by
        // cgb-acid2 and is also explicit in mGBA's software renderer.
        if (lcdc & 0x01) != 0 || mmu.cgb {
            let map_base: u16 = if (lcdc & 0x08) != 0 { 0x1C00 } else { 0x1800 };
            let signed_tiles = (lcdc & 0x10) == 0;

            let y = ly.wrapping_add(mmu.read_byte(SCY_ADDR));
            let tile_row = (y >> 3) as u16;
            let fine_y = y & 0x07;
            let scroll_x = mmu.read_byte(SCX_ADDR);

            for x in 0..160usize {
                let sx = (x as u8).wrapping_add(scroll_x);
                let map_off = map_base + (tile_row << 5) + (sx >> 3) as u16;
                // CGB tile numbers always come from VRAM bank 0; VBK only
                // selects the CPU-visible data bank, not the PPU map source.
                let tile = mmu.read_vram_bank(0, 0x8000 + map_off);
                let attr = if mmu.cgb {
                    mmu.read_vram_bank(1, 0x8000 + map_off)
                } else {
                    0
                };
                let tile_off = tile_data_offset(tile, signed_tiles);
                let tile_y = if attr & 0x40 != 0 { 7 - fine_y } else { fine_y };
                let line_off = tile_off + (tile_y as u16) * 2;
                let bank = if attr & 0x08 != 0 { 1 } else { 0 };
                let b1 = mmu.read_vram_bank(bank, 0x8000 + (line_off & 0x1FFF));
                let b2 = mmu.read_vram_bank(bank, 0x8000 + ((line_off + 1) & 0x1FFF));

                let bit = if attr & 0x20 != 0 {
                    sx & 0x07
                } else {
                    7 - (sx & 0x07)
                };
                let color_num = (((b2 >> bit) & 1) << 1) | ((b1 >> bit) & 1);

                self.bg_color_num[x] = color_num;
                self.bg_priority[x] = mmu.cgb && lcdc & 0x01 != 0 && attr & 0x80 != 0;
                self.buffer[base + x] = if mmu.cgb {
                    Self::cgb_pixel(mmu, false, attr & 0x07, color_num)
                } else {
                    Self::dmg_pixel(bgp, color_num)
                };
            }

            // Window.
            if (lcdc & 0x20) != 0 && !self.window_blocked {
                let wy = mmu.read_byte(WY_ADDR);
                if wy <= ly {
                    let wx = mmu.read_byte(WX_ADDR) as i16 - 7;
                    if wx < 160 {
                        let win_map_base: u16 = if (lcdc & 0x40) != 0 { 0x1C00 } else { 0x1800 };
                        let y_win = self.window_line;
                        let tile_row_win = (y_win >> 3) as u16;
                        let fine_y_win = y_win & 0x07;

                        let start_x = wx.max(0) as usize;
                        for x in start_x..160 {
                            let sxw = (x as i16 - wx) as u16;
                            let map_off =
                                win_map_base + (tile_row_win << 5) + ((sxw as u8) >> 3) as u16;
                            let tile = mmu.read_vram_bank(0, 0x8000 + map_off);
                            let attr = if mmu.cgb {
                                mmu.read_vram_bank(1, 0x8000 + map_off)
                            } else {
                                0
                            };
                            let tile_off = tile_data_offset(tile, signed_tiles);
                            let tile_y = if attr & 0x40 != 0 {
                                7 - fine_y_win
                            } else {
                                fine_y_win
                            };
                            let line_off = tile_off + (tile_y as u16) * 2;
                            let bank = if attr & 0x08 != 0 { 1 } else { 0 };
                            let b1 = mmu.read_vram_bank(bank, 0x8000 + (line_off & 0x1FFF));
                            let b2 = mmu.read_vram_bank(bank, 0x8000 + ((line_off + 1) & 0x1FFF));

                            let bit = if attr & 0x20 != 0 {
                                (sxw as u8) & 0x07
                            } else {
                                7 - ((sxw as u8) & 0x07)
                            };
                            let color_num = (((b2 >> bit) & 1) << 1) | ((b1 >> bit) & 1);

                            self.bg_color_num[x] = color_num;
                            self.bg_priority[x] = mmu.cgb && lcdc & 0x01 != 0 && attr & 0x80 != 0;
                            self.buffer[base + x] = if mmu.cgb {
                                Self::cgb_pixel(mmu, false, attr & 0x07, color_num)
                            } else {
                                Self::dmg_pixel(bgp, color_num)
                            };
                        }
                        self.window_line = self.window_line.wrapping_add(1);
                    }
                }
            }
        }

        // Sprites.
        let obj_enabled =
            (lcdc & 0x02) != 0 || (mmu.cgb && self.window_blocked && self.window_obj_enabled);
        if obj_enabled {
            let height: u8 = if (lcdc & 0x04) != 0 { 16 } else { 8 };

            // OAM search: up to 10 sprites intersecting this line.
            let mut visible: Vec<(u8, u8)> = Vec::with_capacity(10);
            for i in 0..40u8 {
                let sy = mmu.read_byte(0xFE00 + i as u16 * 4).wrapping_sub(16);
                if ly >= sy && ly < sy.saturating_add(height) {
                    visible.push((i, mmu.read_byte(0xFE00 + i as u16 * 4 + 1)));
                    if visible.len() == 10 {
                        break;
                    }
                }
            }

            // DMG priority: lower X wins, ties broken by OAM index. CGB
            // priority is OAM order, so keep the scan's ascending index order.
            if !mmu.cgb || mmu.opri & 1 != 0 {
                visible.sort_by_key(|&(i, x)| (x, i));
            }

            for (i, _) in visible {
                let oam = 0xFE00 + i as u16 * 4;
                let sy = mmu.read_byte(oam).wrapping_sub(16);
                let sx = mmu.read_byte(oam + 1).wrapping_sub(8);
                let mut tile = mmu.read_byte(oam + 2);
                let attr = mmu.read_byte(oam + 3);

                if height == 16 {
                    tile &= 0xFE;
                }

                let mut row = ly.wrapping_sub(sy);
                if (attr & 0x40) != 0 {
                    row = height - 1 - row;
                }

                let tile_off = (tile as u16) << 4;
                let line_off = tile_off + (row as u16) * 2;
                let bank = if mmu.cgb && attr & 0x08 != 0 { 1 } else { 0 };
                let b1 = mmu.read_vram_bank(bank, 0x8000 + (line_off & 0x1FFF));
                let b2 = mmu.read_vram_bank(bank, 0x8000 + ((line_off + 1) & 0x1FFF));

                let palette = if mmu.cgb {
                    attr & 0x07
                } else if (attr & 0x10) != 0 {
                    mmu.read_byte(OBP1_ADDR)
                } else {
                    mmu.read_byte(OBP0_ADDR)
                };

                for px in 0..8u8 {
                    let x = sx.wrapping_add(px);
                    if x >= 160 {
                        continue;
                    }
                    let x = x as usize;
                    if self.sprite_drawn[x] {
                        continue;
                    }

                    let fx = if (attr & 0x20) != 0 { 7 - px } else { px };
                    let bit = 7 - fx;
                    let color_num = (((b2 >> bit) & 1) << 1) | ((b1 >> bit) & 1);
                    let color_num = color_num & 0x03;
                    if color_num == 0 {
                        continue;
                    }

                    // A higher-priority sprite still blocks lower ones even
                    // when it loses to the background.
                    self.sprite_drawn[x] = true;

                    if self.bg_color_num[x] != 0
                        && (!mmu.cgb || lcdc & 0x01 != 0)
                        && ((attr & 0x80) != 0 || (mmu.cgb && self.bg_priority[x]))
                    {
                        continue;
                    }

                    self.buffer[base + x] = if mmu.cgb {
                        Self::cgb_pixel(mmu, true, palette, color_num)
                    } else {
                        Self::dmg_pixel(palette, color_num)
                    };
                }
            }
        }
    }

    /// Clear the framebuffer to the default colour. Kept for API compatibility;
    /// real rendering is performed by [`Ppu::tick`].
    pub fn render_frame(&mut self) {
        for pixel in self.buffer.iter_mut() {
            *pixel = Pixel::default();
        }
    }

    /// Return a slice reference to the current framebuffer.
    pub fn framebuffer(&self) -> &[Pixel] {
        &self.buffer
    }

    /// Consume the frame-completion flag. Returns `true` once per full frame
    /// (when LY wraps from 153 back to 0).
    pub fn take_frame_ready(&mut self) -> bool {
        std::mem::take(&mut self.frame_ready)
    }
}

/// VRAM offset of the first byte of a tile row for tile index `tile`.
/// `signed_tiles` selects the 0x8800 addressing mode where tile indices are
/// signed and the base is 0x9000.
fn tile_data_offset(tile: u8, signed_tiles: bool) -> u16 {
    if signed_tiles {
        let signed = tile as i8 as i32;
        (0x1000 + signed * 16) as u16
    } else {
        (tile as u16) << 4
    }
}
