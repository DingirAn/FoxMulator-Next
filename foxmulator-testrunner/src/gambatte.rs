//! Gambatte-compatible checkpoint runner.
//!
//! Gambatte evaluates ROMs after the fifteenth completed LCD frame. This
//! binary deliberately stops on `frame_ready` rather than on an instruction
//! or T-cycle approximation and writes only artifacts for an external,
//! reference-based evaluator.
use foxmulator_core::system::System;
use std::env;
use std::fs;
use std::path::Path;
use std::time::{Duration, Instant};

fn gambatte_cgb_rgb(pixel: u32) -> [u8; 3] {
    let channel = |value: u8| -> u32 {
        (0..=31)
            .min_by_key(|component| {
                (((*component * 255 + 15) / 31) as i32 - i32::from(value)).unsigned_abs()
            })
            .unwrap()
    };
    let r = channel((pixel >> 16) as u8);
    let g = channel((pixel >> 8) as u8);
    let b = channel(pixel as u8);
    [
        ((r * 13 + g * 2 + b) >> 1) as u8,
        ((g * 3 + b) << 1) as u8,
        ((r * 3 + g * 2 + b * 11) >> 1) as u8,
    ]
}

fn write_ppm(path: &Path, system: &System, cgb: bool) -> std::io::Result<()> {
    let mut data = b"P6\n160 144\n255\n".to_vec();
    for pixel in system.framebuffer() {
        let bytes = pixel.0.to_be_bytes();
        if cgb {
            data.extend_from_slice(&gambatte_cgb_rgb(pixel.0));
        } else {
            let rgb = match [bytes[1], bytes[2], bytes[3]] {
                [0xE0, 0xF8, 0xD0] => [255, 255, 255],
                [0x88, 0xC0, 0x70] => [170, 170, 170],
                [0x34, 0x68, 0x56] => [85, 85, 85],
                [0x08, 0x18, 0x20] => [0, 0, 0],
                other => other,
            };
            data.extend_from_slice(&rgb);
        }
    }
    fs::write(path, data)
}

fn main() {
    let mut args = env::args().skip(1);
    let Some(rom_path) = args.next() else {
        eprintln!("usage: gambatte-runner <rom> <dmg|cgb> <ppm-output> [timeout-secs]");
        std::process::exit(2);
    };
    let Some(model) = args.next() else {
        eprintln!("missing model: dmg or cgb");
        std::process::exit(2);
    };
    let Some(ppm_path) = args.next() else {
        eprintln!("missing PPM output path");
        std::process::exit(2);
    };
    let timeout_secs = args.next().and_then(|v| v.parse().ok()).unwrap_or(8u64);
    let cgb = match model.as_str() {
        "dmg" => false,
        "cgb" => true,
        _ => {
            eprintln!("model must be dmg or cgb");
            std::process::exit(2);
        }
    };
    let boot_trace = env::var_os("FOXMOULATOR_BOOT_TRACE").is_some();
    let rom = fs::read(&rom_path).unwrap_or_else(|err| {
        eprintln!("cannot read ROM: {err}");
        std::process::exit(2);
    });
    let mut system = System::from_rom_with_embedded_boot(rom).unwrap_or_else(|err| {
        eprintln!("cannot load integrated SameBoy boot ROM: {err}");
        std::process::exit(2);
    });
    let start = Instant::now();
    let mut cycles = 0u64;
    let mut frames = 0u32;
    // Gambatte's original runner discards the hardware boot-ROM duration
    // before evaluating the 15-frame test window.
    let boot_frames = if cgb { 186 } else { 334 };
    let extra_frames = env::var("FOXMOULATOR_GAMBATTE_EXTRA_FRAMES")
        .ok()
        .and_then(|value| value.parse::<u32>().ok())
        .unwrap_or(0);
    let evaluation_end = boot_frames + 15 + extra_frames;
    let mut audio_samples = 0usize;
    let mut audio_nonconstant = false;
    let mut previous_sample = None;
    let mut boot_trace_steps = 0usize;
    let trace_dir = env::var_os("FOXMOULATOR_GAMBATTE_TRACE_DIR").map(std::path::PathBuf::from);
    if let Some(dir) = &trace_dir {
        fs::create_dir_all(dir).unwrap_or_else(|err| {
            eprintln!("cannot create trace directory: {err}");
            std::process::exit(2);
        });
    }
    while frames < evaluation_end {
        if start.elapsed() >= Duration::from_secs(timeout_secs) {
            println!(
                "status=TIMEOUT frames={frames} cycles={cycles} pc={:04X} sp={:04X} ff50={:02X} ly={:02X} locked={}",
                system.cpu.pc,
                system.cpu.regs.sp(),
                system.mmu.read_byte(0xFF50),
                system.mmu.read_byte(0xFF44),
                system.cpu.locked
            );
            std::process::exit(1);
        }
        if boot_trace && boot_trace_steps < 2_000_000 {
            let pc = system.cpu.pc;
            if cycles < 2_000 || (0x0100..=0x01D3).contains(&pc) || (0x0280..=0x0340).contains(&pc)
            {
                eprintln!(
                    "BOOT_TRACE step={} cycles={} pc={pc:04X} op={:02X} af={:02X}{:02X} bc={:02X}{:02X} de={:02X}{:02X} hl={:02X}{:02X} sp={:04X} ime={} halted={}",
                    boot_trace_steps,
                    cycles,
                    system.mmu.cpu_read_byte(pc),
                    system.cpu.regs.a,
                    system.cpu.regs.f,
                    system.cpu.regs.b,
                    system.cpu.regs.c,
                    system.cpu.regs.d,
                    system.cpu.regs.e,
                    system.cpu.regs.h,
                    system.cpu.regs.l,
                    system.cpu.regs.sp(),
                    system.cpu.ime,
                    system.cpu.halted,
                );
            }
            boot_trace_steps += 1;
        }
        cycles += u64::from(system.step_instruction());
        for sample in system.take_audio_samples() {
            if let Some(previous) = previous_sample {
                audio_nonconstant |= previous != sample;
            }
            previous_sample = Some(sample);
            audio_samples += 1;
        }
        if system.ppu.take_frame_ready() {
            frames += 1;
            if frames > boot_frames {
                if let Some(dir) = &trace_dir {
                    let path = dir.join(format!("frame-{frames:02}.ppm"));
                    write_ppm(&path, &system, cgb).unwrap_or_else(|err| {
                        eprintln!("cannot write trace PPM: {err}");
                        std::process::exit(2);
                    });
                    println!(
                    "FRAME_TRACE frame={frames} eval_frame={} pc={:04X} ly={:02X} mode={} lcdc={:02X} stat={:02X} scx={:02X} scy={:02X} win_line={} vbk={} bgpal={:04X},{:04X},{:04X},{:04X} map_nz={} map1_nz={} tile_nz={} tile1_nz={} oam_nz={} map0={:02X} tile9000={:02X}{:02X}",
                    frames - boot_frames,
                    system.cpu.pc,
                    system.mmu.read_byte(0xFF44),
                    system.mmu.ppu_mode,
                    system.mmu.read_byte(0xFF40),
                    system.mmu.read_byte(0xFF41),
                    system.mmu.read_byte(0xFF43),
                    system.mmu.read_byte(0xFF42),
                    system.ppu.window_line,
                    system.mmu.vram_bank,
                    system.mmu.cgb_palette_word(false, 0, 0),
                    system.mmu.cgb_palette_word(false, 0, 1),
                    system.mmu.cgb_palette_word(false, 0, 2),
                    system.mmu.cgb_palette_word(false, 0, 3),
                    (0x9800..=0x9FFF)
                        .filter(|&addr| system.mmu.read_vram_bank(0, addr) != 0)
                        .count(),
                    (0x9800..=0x9FFF)
                        .filter(|&addr| system.mmu.read_vram_bank(1, addr) != 0)
                        .count(),
                    (0x8000..=0x97FF)
                        .filter(|&addr| system.mmu.read_vram_bank(0, addr) != 0)
                        .count(),
                    (0x8000..=0x97FF)
                        .filter(|&addr| system.mmu.read_vram_bank(1, addr) != 0)
                        .count(),
                    (0xFE00..=0xFE9F)
                        .filter(|&addr| system.mmu.read_byte(addr) != 0)
                        .count(),
                    system.mmu.read_vram_bank(0, 0x9800),
                    system.mmu.read_vram_bank(0, 0x9000),
                    system.mmu.read_vram_bank(0, 0x9001),
                    );
                }
            }
        }
    }
    if let Err(err) = write_ppm(Path::new(&ppm_path), &system, cgb) {
        eprintln!("cannot write PPM: {err}");
        std::process::exit(2);
    }
    println!(
        "status=PASS frames={frames} cycles={cycles} model={model} pc={:04X} ff50={:02X} locked={} audio_samples={audio_samples} audio_nonconstant={audio_nonconstant}",
        system.cpu.pc,
        system.mmu.read_byte(0xFF50),
        system.cpu.locked
    );
}
