//! Comparable headless CPU/APU pacing measurement.
//!
//! This intentionally does not claim to validate audible output. It separates
//! emulator/APU work from GUI/device work by measuring the same ROM and cycle
//! budget with and without draining the generated PCM queue.

use foxmulator_core::system::{LinkCable, System};
use std::env;
use std::fs;
use std::time::{Duration, Instant};

const DEFAULT_TIMEOUT_SECS: u64 = 35;

fn run_single(rom: &[u8], cycles: u64, drain_audio: bool, deadline: Instant) -> Option<(u64, u64)> {
    let mut system =
        System::from_rom_with_embedded_boot(rom.to_vec()).expect("integrated SameBoy boot ROM");
    let mut executed = 0u64;
    let mut samples = 0u64;
    while executed < cycles {
        if Instant::now() >= deadline {
            return None;
        }
        executed += u64::from(system.step_instruction());
        if drain_audio {
            samples += system.take_audio_samples().len() as u64;
        }
    }
    Some((executed, samples))
}

fn run_dual(rom: &[u8], cycles: u64, deadline: Instant) -> Option<u64> {
    let mut left = System::from_rom_with_embedded_boot(rom.to_vec())
        .expect("integrated SameBoy left boot ROM");
    let mut right = System::from_rom_with_embedded_boot(rom.to_vec())
        .expect("integrated SameBoy right boot ROM");
    let mut left_cycles = 0u64;
    let mut right_cycles = 0u64;
    let mut cable = LinkCable::new();
    while left_cycles < cycles || right_cycles < cycles {
        if Instant::now() >= deadline {
            return None;
        }
        if left_cycles < cycles {
            left_cycles += u64::from(left.step_instruction());
        }
        if right_cycles < cycles {
            right_cycles += u64::from(right.step_instruction());
        }
        cable.pump(&mut left, &mut right);
    }
    Some(left_cycles.max(right_cycles))
}

fn main() {
    let mut args = env::args().skip(1);
    let Some(path) = args.next() else {
        eprintln!("usage: audio-benchmark ROM [cycles] [timeout-secs]");
        std::process::exit(2);
    };
    let cycles = args
        .next()
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(10_000_000);
    let timeout_secs = args
        .next()
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(DEFAULT_TIMEOUT_SECS);
    let rom = fs::read(&path).expect("ROM readable");
    let model = if System::cgb_mode_for_rom(&rom) {
        "CGB"
    } else {
        "DMG"
    };

    for (label, drain) in [("headless-no-drain", false), ("headless-pcm-drain", true)] {
        let started = Instant::now();
        let deadline = started + Duration::from_secs(timeout_secs);
        let Some((executed, samples)) = run_single(&rom, cycles, drain, deadline) else {
            eprintln!("TIMEOUT(time): audio benchmark mode={label} limit={timeout_secs}s");
            std::process::exit(1);
        };
        let seconds = started.elapsed().as_secs_f64();
        let emulated_seconds = executed as f64 / 4_194_304.0;
        println!(
            "rom={} model={} mode={} cycles={} wall_ms={:.3} emu_mcycles_s={:.3} pcm_samples={} pcm_wall_hz={:.1} pcm_emulated_hz={:.1}",
            path,
            model,
            label,
            executed,
            seconds * 1000.0,
            executed as f64 / seconds / 1_000_000.0,
            samples,
            samples as f64 / seconds,
            samples as f64 / emulated_seconds,
        );
    }

    let started = Instant::now();
    let deadline = started + Duration::from_secs(timeout_secs);
    let Some(executed) = run_dual(&rom, cycles, deadline) else {
        eprintln!("TIMEOUT(time): audio benchmark mode=dual-no-gui limit={timeout_secs}s");
        std::process::exit(1);
    };
    let seconds = started.elapsed().as_secs_f64();
    println!(
        "rom={} model={} mode=dual-no-gui cycles={} wall_ms={:.3} emu_mcycles_s={:.3}",
        path,
        model,
        executed,
        seconds * 1000.0,
        executed as f64 / seconds / 1_000_000.0,
    );
}
