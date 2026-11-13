//! Runner for ROM suites whose result is exposed through CPU registers or RAM.
use foxmulator_core::system::System;
use std::env;
use std::fs;
use std::path::Path;
use std::time::{Duration, Instant};

fn ppm(path: &Path, system: &System) -> std::io::Result<()> {
    let mut out = b"P6\n160 144\n255\n".to_vec();
    for pixel in system.framebuffer() {
        let bytes = pixel.0.to_be_bytes();
        out.extend_from_slice(&bytes[1..4]);
    }
    fs::write(path, out)
}

fn main() {
    let mut args = env::args().skip(1);
    let Some(path) = args.next() else {
        eprintln!("usage: compat-runner <rom-or-directory> [--cycles N] [--timeout-secs N] [--ppm-dir DIR]");
        std::process::exit(2);
    };
    if Path::new(&path)
        .components()
        .any(|component| component.as_os_str() == "oam_bug")
    {
        eprintln!("oam_bug is excluded from the hardware compatibility campaign");
        std::process::exit(2);
    }
    let mut max_cycles = 0u64;
    let mut timeout_secs = 15u64;
    let mut ppm_dir = None;
    let mut pending = None;
    for arg in args {
        if let Some(key) = pending.take() {
            match key {
                "cycles" => max_cycles = arg.parse().unwrap_or(max_cycles),
                "timeout-secs" => timeout_secs = arg.parse().unwrap_or(timeout_secs),
                "ppm-dir" => ppm_dir = Some(arg),
                _ => {}
            }
        } else if arg == "--cycles" {
            pending = Some("cycles");
        } else if arg == "--timeout-secs" {
            pending = Some("timeout-secs");
        } else if arg == "--ppm-dir" {
            pending = Some("ppm-dir");
        } else {
            eprintln!("unknown argument: {arg}");
            std::process::exit(2);
        }
    }
    let paths = if Path::new(&path).is_file() {
        vec![path]
    } else {
        let mut v = Vec::new();
        fn walk(p: &Path, v: &mut Vec<String>) {
            if p.file_name().is_some_and(|name| name == "oam_bug") {
                return;
            }
            if let Ok(entries) = fs::read_dir(p) {
                for e in entries.flatten() {
                    let p = e.path();
                    if p.is_dir() {
                        if p.file_name().is_some_and(|name| name == "oam_bug") {
                            continue;
                        }
                        walk(&p, v);
                    } else if matches!(p.extension().and_then(|x| x.to_str()), Some("gb" | "gbc")) {
                        v.push(p.display().to_string());
                    }
                }
            }
        }
        walk(Path::new(&path), &mut v);
        v.sort();
        v
    };
    if let Some(dir) = &ppm_dir {
        fs::create_dir_all(dir).expect("create ppm directory");
    }
    let mut counts = [0usize; 5];
    for path in paths {
        let rom = match fs::read(&path) {
            Ok(x) => x,
            Err(e) => {
                println!("ERROR {path} {e}");
                counts[4] += 1;
                continue;
            }
        };
        let mut system =
            System::from_rom_with_embedded_boot(rom.clone()).expect("integrated SameBoy boot ROM");
        let start = Instant::now();
        let mut cycles = 0u64;
        let mut result = None;
        while cycles < max_cycles || max_cycles == 0 {
            if start.elapsed() >= Duration::from_secs(timeout_secs) {
                break;
            }
            let pc = system.cpu.pc;
            let op = system.mmu.read_byte(pc);
            if op == 0xED {
                result = Some(
                    if system.cpu.regs.bc() == 0x0305
                        && system.cpu.regs.de() == 0x080D
                        && system.cpu.regs.hl() == 0x1522
                    {
                        "PASS"
                    } else {
                        "FAIL"
                    },
                );
                break;
            }
            if op == 0x40
                && system.cpu.regs.bc() == 0x0305
                && system.cpu.regs.de() == 0x080D
                && system.cpu.regs.hl() == 0x1522
            {
                result = Some("PASS");
                break;
            }
            cycles += u64::from(system.step_instruction());
            if system.mmu.read_byte(0xFF82) == 1 {
                result = Some("PASS");
                break;
            }
            if system.mmu.read_byte(0xFF82) == 0xFF {
                result = Some("FAIL");
                break;
            }
        }
        let label = result.unwrap_or(if max_cycles != 0 && cycles >= max_cycles {
            "TIMEOUT(cycles)"
        } else {
            "TIMEOUT(time)"
        });
        let idx = match label {
            "PASS" => 0,
            "FAIL" => 1,
            x if x.starts_with("TIMEOUT") => 2,
            _ => 4,
        };
        counts[idx] += 1;
        if let Some(dir) = &ppm_dir {
            let name = Path::new(&path).file_name().unwrap().to_string_lossy();
            let _ = ppm(&Path::new(dir).join(format!("{name}.ppm")), &system);
        }
        println!(
            "{label} {path} cycles={cycles} pc={:04X} bc={:04X} de={:04X} hl={:04X} ff80={:02X} ff81={:02X} ff82={:02X}",
            system.cpu.pc,
            system.cpu.regs.bc(),
            system.cpu.regs.de(),
            system.cpu.regs.hl(),
            system.mmu.read_byte(0xFF80),
            system.mmu.read_byte(0xFF81),
            system.mmu.read_byte(0xFF82)
        );
    }
    println!(
        "TOTAL pass={} fail={} timeout={} error={}",
        counts[0], counts[1], counts[2], counts[4]
    );
    if counts[1] + counts[2] + counts[4] != 0 {
        std::process::exit(1);
    }
}
