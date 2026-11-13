//! Small headless runner used by scripts/qa.sh.
//!
//! It exercises the real System and LinkCable types without opening a GUI. It
//! deliberately reports bounded startup/progress evidence, not commercial ROM
//! playability.

use foxmulator_core::system::{LinkCable, System};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Scenario {
    Single,
    Dual,
    RightOnly,
}

#[derive(Debug)]
struct Options {
    scenario: Scenario,
    left: Option<PathBuf>,
    right: Option<PathBuf>,
    max_cycles: u64,
    timeout: Duration,
}

fn usage() -> ! {
    eprintln!(
        "usage: qa-runner --scenario single|dual|right-only [--left ROM] [--right ROM] [--max-cycles N] [--timeout-secs N]"
    );
    std::process::exit(2);
}

fn next_value<I: Iterator<Item = String>>(name: &str, args: &mut I) -> String {
    args.next().unwrap_or_else(|| {
        eprintln!("missing value for {name}");
        usage();
    })
}

fn parse_options() -> Options {
    let mut scenario = None;
    let mut left = None;
    let mut right = None;
    let mut max_cycles = 1_000_000;
    let mut timeout = 15;
    let mut args = env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--scenario" => {
                scenario = Some(match next_value("--scenario", &mut args).as_str() {
                    "single" => Scenario::Single,
                    "dual" => Scenario::Dual,
                    "right-only" => Scenario::RightOnly,
                    other => {
                        eprintln!("unknown scenario: {other}");
                        usage();
                    }
                });
            }
            "--left" => left = Some(PathBuf::from(next_value("--left", &mut args))),
            "--right" => right = Some(PathBuf::from(next_value("--right", &mut args))),
            "--max-cycles" => {
                max_cycles = next_value("--max-cycles", &mut args)
                    .parse()
                    .unwrap_or_else(|_| {
                        eprintln!("invalid --max-cycles");
                        usage();
                    });
            }
            "--timeout-secs" => {
                timeout = next_value("--timeout-secs", &mut args)
                    .parse()
                    .unwrap_or_else(|_| {
                        eprintln!("invalid --timeout-secs");
                        usage();
                    });
            }
            "--help" | "-h" => usage(),
            other => {
                eprintln!("unexpected argument: {other}");
                usage();
            }
        }
    }
    let Some(scenario) = scenario else { usage() };
    match scenario {
        Scenario::Single if left.is_none() => {
            eprintln!("single requires --left ROM");
            usage();
        }
        Scenario::Dual if left.is_none() || right.is_none() => {
            eprintln!("dual requires --left ROM and --right ROM");
            usage();
        }
        Scenario::RightOnly if right.is_none() => {
            eprintln!("right-only requires --right ROM");
            usage();
        }
        _ => {}
    }
    Options {
        scenario,
        left,
        right,
        max_cycles,
        timeout: Duration::from_secs(timeout),
    }
}

fn load(path: &Path) -> (Vec<u8>, bool) {
    let data = fs::read(path).unwrap_or_else(|error| {
        eprintln!("cannot read {}: {error}", path.display());
        std::process::exit(1);
    });
    if data.len() < 0x150 {
        eprintln!(
            "ROM is too small for a cartridge header: {}",
            path.display()
        );
        std::process::exit(1);
    }
    let cgb = System::cgb_mode_for_rom(&data);
    (data, cgb)
}

fn one(system: &mut System, max_cycles: u64, timeout: Duration) -> (u64, u64, u64, u16) {
    let started = Instant::now();
    let mut cycles = 0;
    let mut frames = 0;
    let mut mode_changes = 0;
    let mut previous_mode = system.mmu.ppu_mode;
    while cycles < max_cycles {
        if started.elapsed() >= timeout {
            eprintln!("TIMEOUT single cycles={cycles}");
            std::process::exit(1);
        }
        cycles += u64::from(system.step_instruction());
        if system.mmu.ppu_mode != previous_mode {
            mode_changes += 1;
            previous_mode = system.mmu.ppu_mode;
        }
        if system.ppu.take_frame_ready() {
            frames += 1;
        }
    }
    if mode_changes == 0 {
        eprintln!("FAIL single: no PPU mode progress after {cycles} cycles");
        std::process::exit(1);
    }
    (cycles, frames, mode_changes, system.cpu.pc)
}

fn dual(
    left: &mut System,
    right: &mut System,
    max_cycles: u64,
    timeout: Duration,
) -> (u64, u64, u64, u64, u64, u16, u16) {
    let started = Instant::now();
    let mut cable = LinkCable::new();
    let mut left_cycles = 0;
    let mut right_cycles = 0;
    let mut left_frames = 0;
    let mut right_frames = 0;
    let mut left_modes = 0;
    let mut right_modes = 0;
    let mut left_previous_mode = left.mmu.ppu_mode;
    let mut right_previous_mode = right.mmu.ppu_mode;
    while left_cycles < max_cycles || right_cycles < max_cycles {
        if started.elapsed() >= timeout {
            eprintln!("TIMEOUT dual cycles={left_cycles}/{right_cycles}");
            std::process::exit(1);
        }
        if left_cycles < max_cycles {
            left_cycles += u64::from(left.step_instruction());
        }
        if right_cycles < max_cycles {
            right_cycles += u64::from(right.step_instruction());
        }
        cable.pump(left, right);
        if left.mmu.ppu_mode != left_previous_mode {
            left_modes += 1;
            left_previous_mode = left.mmu.ppu_mode;
        }
        if right.mmu.ppu_mode != right_previous_mode {
            right_modes += 1;
            right_previous_mode = right.mmu.ppu_mode;
        }
        if left.ppu.take_frame_ready() {
            left_frames += 1;
        }
        if right.ppu.take_frame_ready() {
            right_frames += 1;
        }
    }
    if left_modes == 0 || right_modes == 0 {
        eprintln!("FAIL dual: no PPU mode progress left={left_modes} right={right_modes}");
        std::process::exit(1);
    }
    (
        left_cycles + right_cycles,
        left_frames,
        right_frames,
        left_modes,
        right_modes,
        left.cpu.pc,
        right.cpu.pc,
    )
}

fn main() {
    let options = parse_options();
    match options.scenario {
        Scenario::Single => {
            let path = options.left.as_deref().expect("validated left ROM");
            let (data, cgb) = load(path);
            let mut system = System::from_rom_with_embedded_boot(data.clone())
                .expect("integrated SameBoy boot ROM");
            let (cycles, frames, modes, pc) = one(&mut system, options.max_cycles, options.timeout);
            println!(
                "QA scenario=single result=PASS path={:?} bytes={} model={} cycles={} frames={} ppu_mode_changes={} pc={pc:04X}",
                path,
                data.len(),
                if cgb { "GBC" } else { "DMG" },
                cycles,
                frames,
                modes
            );
        }
        Scenario::RightOnly => {
            let path = options.right.as_deref().expect("validated right ROM");
            let (data, cgb) = load(path);
            let mut system = System::from_rom_with_embedded_boot(data.clone())
                .expect("integrated SameBoy boot ROM");
            let (cycles, frames, modes, pc) = one(&mut system, options.max_cycles, options.timeout);
            println!(
                "QA scenario=right-only result=PASS path={:?} bytes={} model={} cycles={} frames={} ppu_mode_changes={} pc={pc:04X}",
                path,
                data.len(),
                if cgb { "GBC" } else { "DMG" },
                cycles,
                frames,
                modes
            );
        }
        Scenario::Dual => {
            let left_path = options.left.as_deref().expect("validated left ROM");
            let right_path = options.right.as_deref().expect("validated right ROM");
            let (left_data, left_cgb) = load(left_path);
            let (right_data, right_cgb) = load(right_path);
            let mut left = System::from_rom_with_embedded_boot(left_data.clone())
                .expect("integrated SameBoy left boot ROM");
            let mut right = System::from_rom_with_embedded_boot(right_data.clone())
                .expect("integrated SameBoy right boot ROM");
            let (cycles, left_frames, right_frames, left_modes, right_modes, left_pc, right_pc) =
                dual(&mut left, &mut right, options.max_cycles, options.timeout);
            println!(
                "QA scenario=dual result=PASS left={:?} right={:?} models={}/{} cycles={} frames={}/{} ppu_mode_changes={}/{} pc={left_pc:04X}/{right_pc:04X}",
                left_path,
                right_path,
                if left_cgb { "GBC" } else { "DMG" },
                if right_cgb { "GBC" } else { "DMG" },
                cycles,
                left_frames,
                right_frames,
                left_modes,
                right_modes
            );
        }
    }
}
