//! Headless diagnostics for local commercial ROM compatibility.
//!
//! This runner deliberately does not infer playability from instruction count.
//! It records CPU/PPU activity, optionally injects deterministic diagnostic
//! joypad input, and writes the final framebuffer outside the repository for
//! visual inspection.

use foxmulator_core::joypad::Button;
use foxmulator_core::system::System;
use std::env;
use std::fs;
use std::path::Path;
use std::time::{Duration, Instant};

const DEFAULT_CYCLES: u64 = 8_000_000;
const DEFAULT_TIMEOUT_SECS: u64 = 35;

fn title(rom: &[u8]) -> String {
    rom.get(0x134..0x144)
        .unwrap_or_default()
        .iter()
        .take_while(|b| **b != 0)
        .map(|b| {
            if b.is_ascii_graphic() {
                *b as char
            } else {
                ' '
            }
        })
        .collect::<String>()
        .trim_end()
        .to_string()
}

fn excluded_from_hardware_campaign(path: &Path) -> bool {
    path.components()
        .any(|component| component.as_os_str() == "oam_bug")
}

fn write_ppm(path: &Path, system: &System) -> std::io::Result<()> {
    let mut data = "P6\n160 144\n255\n".to_string().into_bytes();
    for pixel in system.framebuffer() {
        let rgb = pixel.0.to_be_bytes();
        data.extend_from_slice(&rgb[1..4]);
    }
    fs::write(path, data)
}

fn trace_frame(dir: &Path, frame: u64, system: &System) {
    let vram_hash = |bank: u8, start: u16, end: u16| {
        (start..=end).fold(0u16, |hash, addr| {
            hash.wrapping_mul(257)
                .wrapping_add(u16::from(system.mmu.read_vram_bank(bank, addr)))
        })
    };
    let vram_nonzero = |bank: u8, start: u16, end: u16| {
        (start..=end)
            .filter(|&addr| system.mmu.read_vram_bank(bank, addr) != 0)
            .count()
    };
    let path = dir.join(format!("frame-{frame:05}.ppm"));
    if let Err(err) = write_ppm(&path, system) {
        eprintln!(
            "could not write trace framebuffer {}: {err}",
            path.display()
        );
    }
    println!(
        "FRAME_TRACE frame={frame} pc={:04X} sp={:04X} ime={} halted={} if={:02X} ie={:02X} joyp={:02X} lcdc={:02X} stat={:02X} ly={:02X} scx={:02X} scy={:02X} wx={:02X} wy={:02X} win_line={} key1={:02X} rom_bank={} vram_bank={} wram_bank={} speed2={} map0={:04X} map1={:04X} tile0={:04X} tile1={:04X} winmap_nz0={} winmap_nz1={}",
        system.cpu.pc,
        system.cpu.regs.sp(),
        system.cpu.ime,
        system.cpu.halted,
        system.mmu.read_byte(0xFF0F),
        system.mmu.read_byte(0xFFFF),
        system.mmu.read_byte(0xFF00),
        system.mmu.read_byte(0xFF40),
        system.mmu.read_byte(0xFF41),
        system.mmu.read_byte(0xFF44),
        system.mmu.read_byte(0xFF43),
        system.mmu.read_byte(0xFF42),
        system.mmu.read_byte(0xFF4B),
        system.mmu.read_byte(0xFF4A),
        system.ppu.window_line,
        system.mmu.read_byte(0xFF4D),
        system.mmu.current_rom_bank,
        system.mmu.vram_bank,
        system.mmu.wram_bank,
        system.mmu.double_speed,
        vram_hash(0, 0x9800, 0x9FFF),
        vram_hash(1, 0x9800, 0x9FFF),
        vram_hash(0, 0x8000, 0x97FF),
        vram_hash(1, 0x8000, 0x97FF),
        vram_nonzero(0, 0x9C00, 0x9FFF),
        vram_nonzero(1, 0x9C00, 0x9FFF),
    );
    if std::env::var_os("FOXMOULATOR_OAM_TRACE").is_some() {
        for i in 0..40u16 {
            let base = 0xFE00 + i * 4;
            let y = system.mmu.read_byte(base);
            let x = system.mmu.read_byte(base + 1);
            let tile = system.mmu.read_byte(base + 2);
            let attr = system.mmu.read_byte(base + 3);
            if y != 0 || x != 0 || tile != 0 || attr != 0 {
                println!(
                    "OAM_TRACE frame={frame} index={i} y={y:02X} x={x:02X} tile={tile:02X} attr={attr:02X}"
                );
            }
        }
        for (label, x, y) in [("left-arrow", 40u8, 73u8), ("right-arrow", 120u8, 73u8)] {
            let lcdc = system.mmu.read_byte(0xFF40);
            let map = if lcdc & 0x08 != 0 { 0x1C00 } else { 0x1800 };
            let map_addr = 0x8000 + map + (((u16::from(y)) >> 3) << 5) + (u16::from(x) >> 3);
            let tile = system.mmu.read_vram_bank(0, map_addr);
            let attr = system.mmu.read_vram_bank(1, map_addr);
            let row = u16::from(y & 7);
            let tile_addr = 0x8000 + (u16::from(tile) << 4) + row * 2;
            println!(
                "BG_TRACE frame={frame} label={label} map={map_addr:04X} tile={tile:02X} attr={attr:02X} tile_addr={tile_addr:04X} bytes={:02X}{:02X} tile_rows={} pal0={:04X} pal1={:04X} pal2={:04X} pal3={:04X}",
                system.mmu.read_vram_bank(if attr & 8 != 0 { 1 } else { 0 }, tile_addr),
                system.mmu.read_vram_bank(if attr & 8 != 0 { 1 } else { 0 }, tile_addr + 1),
                (0..8)
                    .map(|row| {
                        let addr = tile_addr - u16::from(y & 7) * 2 + row * 2;
                        format!(
                            "{:02X}{:02X}",
                            system.mmu.read_vram_bank(if attr & 8 != 0 { 1 } else { 0 }, addr),
                            system.mmu.read_vram_bank(if attr & 8 != 0 { 1 } else { 0 }, addr + 1)
                        )
                    })
                    .collect::<Vec<_>>()
                    .join("/"),
                system.mmu.cgb_palette_word(false, attr & 7, 0),
                system.mmu.cgb_palette_word(false, attr & 7, 1),
                system.mmu.cgb_palette_word(false, attr & 7, 2),
                system.mmu.cgb_palette_word(false, attr & 7, 3),
            );
        }
    }
}

fn main() {
    let mut args = env::args().skip(1);
    let Some(path) = args.next() else {
        eprintln!("usage: commercial-runner <rom> [max-cycles] [ppm-output] [timeout-secs]");
        std::process::exit(2);
    };
    if excluded_from_hardware_campaign(Path::new(&path)) {
        eprintln!("oam_bug is excluded from the hardware compatibility campaign");
        std::process::exit(2);
    }
    let max_cycles = args
        .next()
        .and_then(|s| s.parse().ok())
        .unwrap_or(DEFAULT_CYCLES);
    let ppm = args.next();
    let timeout_secs = args
        .next()
        .and_then(|s| s.parse().ok())
        .unwrap_or(DEFAULT_TIMEOUT_SECS);
    let rom = fs::read(&path).unwrap_or_else(|err| {
        eprintln!("cannot read {path}: {err}");
        std::process::exit(2);
    });
    if rom.len() < 0x150 {
        eprintln!(
            "ROM is too small for a cartridge header: {} bytes",
            rom.len()
        );
        std::process::exit(2);
    }

    let mut system = System::from_rom_with_embedded_boot(rom.clone()).unwrap_or_else(|error| {
        eprintln!("integrated SameBoy boot ROM failed: {error}");
        std::process::exit(2);
    });
    let mut cycles = 0u64;
    let mut frames = 0u64;
    let mut mode_changes = 0u64;
    let mut interrupts = 0u64;
    let mut last_mode = system.mmu.ppu_mode;
    let mut last_if = system.mmu.read_byte(0xFF0F);
    let scripted_input = env::var("FOXMOULATOR_RUNNER_SCRIPTED_INPUT").ok();
    let trace_dir = env::var_os("FOXMOULATOR_RUNNER_TRACE_DIR").map(std::path::PathBuf::from);
    if let Some(dir) = &trace_dir {
        fs::create_dir_all(dir).expect("create runner trace directory");
    }
    let mut scripted_pressed = false;
    let mut scripted_down_pressed = false;
    let mut scripted_second_a_pressed = false;
    let start = Instant::now();

    let timeout = Duration::from_secs(timeout_secs);
    let mut limit_reason = "cycles";
    while cycles < max_cycles && start.elapsed() < timeout {
        cycles += u64::from(system.step_instruction());
        if system.mmu.ppu_mode != last_mode {
            mode_changes += 1;
            last_mode = system.mmu.ppu_mode;
        }
        let if_reg = system.mmu.read_byte(0xFF0F);
        interrupts += u64::from((if_reg ^ last_if).count_ones());
        last_if = if_reg;
        if system.ppu.take_frame_ready() {
            frames += 1;
            if let Some(dir) = &trace_dir {
                if (850..=1_100).contains(&frames) {
                    trace_frame(dir, frames, &system);
                }
            }
            // Deterministic diagnostic navigation. This is frame-based rather
            // than instruction-cycle-modulo based, so a variable-length CPU
            // instruction cannot skip the transition. `late` is useful for
            // games whose copyright/title sequence takes many frames.
            if scripted_input.is_some() {
                let a_only = matches!(
                    scripted_input.as_deref(),
                    Some("a-late" | "a-very-late" | "a-one" | "a-down" | "a-down-select")
                );
                let very_late = matches!(scripted_input.as_deref(), Some("a-very-late" | "a-one"));
                let one_frame = scripted_input.as_deref() == Some("a-one");
                let late = matches!(
                    scripted_input.as_deref(),
                    Some("late" | "a-late" | "a-very-late" | "a-one" | "a-down" | "a-down-select")
                );
                let start_range = if late { 300..320 } else { 30..40 };
                let action_range = if one_frame {
                    1_000..1_001
                } else if very_late {
                    1_000..1_020
                } else if late {
                    900..920
                } else {
                    60..70
                };
                let should_press = start_range.contains(&frames) || action_range.contains(&frames);
                if should_press != scripted_pressed {
                    system.set_button(
                        Button::Start,
                        should_press && !a_only && frames < action_range.start,
                    );
                    system.set_button(Button::A, should_press && frames >= action_range.start);
                    scripted_pressed = should_press;
                }
                if matches!(scripted_input.as_deref(), Some("a-down" | "a-down-select")) {
                    let down_pressed = (1_050..1_060).contains(&frames);
                    if down_pressed != scripted_down_pressed {
                        system.set_button(Button::Down, down_pressed);
                        scripted_down_pressed = down_pressed;
                    }
                    if scripted_input.as_deref() == Some("a-down-select") {
                        let second_a_pressed = (1_100..1_120).contains(&frames);
                        if second_a_pressed != scripted_second_a_pressed {
                            system.set_button(Button::A, second_a_pressed);
                            scripted_second_a_pressed = second_a_pressed;
                        }
                    }
                }
            }
        }

        // Preserve the historical conservative input exercise by default, but
        // allow compatibility diagnostics to run with a genuinely idle
        // joypad. This is intentionally opt-out so existing runner results do
        // not silently change.
        if env::var_os("FOXMOULATOR_RUNNER_NO_INPUT").is_none() {
            if cycles % 262_144 < 8 {
                system.set_button(Button::Start, true);
                system.set_button(Button::A, true);
            } else if cycles % 262_144 < 16 {
                system.set_button(Button::Start, false);
                system.set_button(Button::A, false);
            }
        }
    }

    if cycles >= max_cycles {
        limit_reason = "cycles";
    } else if start.elapsed() >= timeout {
        limit_reason = "time";
    }

    if let Some(path) = ppm {
        if let Err(err) = write_ppm(Path::new(&path), &system) {
            eprintln!("could not write framebuffer {path}: {err}");
        }
    }

    println!(
        "ROM={} title={:?} bytes={} model={} result={} cycles={} frames={} lcd_mode_changes={} interrupt_edges={} pc={:04X} sp={:04X} halted={} locked={} wall_ms={}",
        path,
        title(&rom),
        rom.len(),
        if System::cgb_mode_for_rom(&rom) {
            "CGB"
        } else {
            "DMG"
        },
        if cycles >= max_cycles || start.elapsed() >= timeout {
            format!("TIMEOUT({limit_reason})")
        } else {
            "COMPLETE".to_string()
        },
        cycles,
        frames,
        mode_changes,
        interrupts,
        system.cpu.pc,
        system.cpu.regs.sp(),
        system.cpu.halted,
        system.cpu.locked,
        start.elapsed().as_millis()
    );
}
