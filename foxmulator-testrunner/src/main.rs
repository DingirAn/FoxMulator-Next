//! Blargg Game Boy test ROM runner.
//!
//! Loads one ROM (or a directory of ROMs), runs the emulator, captures the
//! serial output produced by Blargg's test shell, and classifies the result as
//! PASS, FAIL, TIMEOUT or ERROR.
//!
//! Blargg's ROMs write each output character to SB (0xFF01), then start a
//! serial transfer by writing 0x81 to SC (0xFF02). The shell prints its final
//! verdict as `Passed`, `Failed`, `Failed #n` or, for multi-ROMs,
//! `Passed all tests` / `Failed N tests.`.
//!
//! Usage:
//!   blargg-runner <rom-or-directory> [options]
//!
//! Options:
//!   --timeout-secs N   Per-ROM wall-clock limit (default 35; complex 60).
//!   --max-cycles N     Per-ROM T-cycle budget (default 300,000,000).
//!                      Use 0 for no cycle limit.
//!   --quiet            Do not print periodic progress.
//!   --force-dmg        Run the ROM on DMG hardware (for DMG-specific suites).
//!   --complex          Use the 60-second complex multi-ROM profile.
//!   --control-trace    Log bounded Blargg control-state transitions.
//!
//! Progress is written to stderr and the final table to stdout, so piping the
//! table through `tail`/`head` does not hide the live progress.
//!
//! Exit code is 0 when every executed ROM passed, 1 otherwise.

use foxmulator_core::system::System;
use std::collections::VecDeque;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Status {
    Pass,
    Fail,
    TimeoutWall,
    TimeoutCycles,
    Error,
}

impl Status {
    fn label(self) -> &'static str {
        match self {
            Status::Pass => "PASS",
            Status::Fail => "FAIL",
            Status::TimeoutWall => "TIMEOUT(time)",
            Status::TimeoutCycles => "TIMEOUT(cycles)",
            Status::Error => "ERROR",
        }
    }
}

struct Outcome {
    name: String,
    status: Status,
    cycles: u64,
    wall: Duration,
    message: String,
}

struct RunOptions {
    max_cycles: u64,
    timeout: Duration,
    quiet: bool,
    dump_tail: usize,
    stop_on_halt: bool,
    watch: Option<u16>,
    force_dmg: bool,
    control_trace: bool,
}

const DEFAULT_MAX_CYCLES: u64 = 300_000_000;
const DEFAULT_TIMEOUT_SECS: u64 = 35;
const COMPLEX_TIMEOUT_SECS: u64 = 60;

/// Inspect the accumulated serial output for a terminal Blargg verdict.
fn verdict(output: &[u8]) -> Option<bool> {
    let text = String::from_utf8_lossy(output);
    if text.contains("Passed all tests") {
        return Some(true);
    }
    if text.contains("Failed ") && text.contains("tests.") {
        return Some(false);
    }
    // Individual verdicts are printed at the end. Only examine the tail.
    let tail: Vec<&str> = text
        .lines()
        .rev()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .take(3)
        .collect();
    for line in tail {
        if line == "Passed" || line == "Done" {
            return Some(true);
        }
        if line == "Failed" || line.starts_with("Failed #") || line == "Internal error" {
            return Some(false);
        }
    }
    None
}

fn last_message(output: &[u8]) -> String {
    let text = String::from_utf8_lossy(output);
    text.lines()
        .rev()
        .find(|l| !l.trim().is_empty())
        .unwrap_or("")
        .to_string()
}

fn stop_on_halt_requested(enabled: bool, opcode: u8, halted: bool) -> bool {
    enabled && opcode == 0x76 && halted
}

fn select_cgb_mode(rom: &[u8], force_dmg: bool) -> bool {
    !force_dmg && System::cgb_mode_for_rom(rom)
}

/// Blargg's memory output protocol (used by ROMs that do not print via the
/// serial port). `$A000` holds the status (`$80` while running, then the final
/// result code: 0 = passed, 1 = failed, 2+ = specific reason). `$A001-$A003`
/// hold the signature `$DE,$B0,$61` and the text is a zero-terminated string at
/// `$A004`. Reference: `mem_timing-2/readme.txt`, "Output to memory".
fn memory_signature_present(system: &System) -> bool {
    system.mmu.read_byte(0xA001) == 0xDE
        && system.mmu.read_byte(0xA002) == 0xB0
        && system.mmu.read_byte(0xA003) == 0x61
}

fn memory_text(system: &System) -> String {
    let mut text = String::new();
    for offset in 0..4096u16 {
        let byte = system.mmu.read_byte(0xA004 + offset);
        if byte == 0 {
            break;
        }
        text.push(byte as char);
    }
    text
}

fn run_rom(path: &Path, options: &RunOptions) -> Outcome {
    let name = path
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| path.display().to_string());

    let data = match std::fs::read(path) {
        Ok(data) => data,
        Err(err) => {
            return Outcome {
                name,
                status: Status::Error,
                cycles: 0,
                wall: Duration::ZERO,
                message: format!("cannot read ROM: {err}"),
            }
        }
    };

    let cgb = select_cgb_mode(&data, options.force_dmg);
    let mut system = match System::from_model_with_embedded_boot(data, cgb) {
        Ok(system) => system,
        Err(error) => {
            return Outcome {
                name,
                status: Status::Error,
                cycles: 0,
                wall: Duration::ZERO,
                message: format!("integrated SameBoy boot ROM failed: {error}"),
            }
        }
    };
    if options.control_trace {
        system.mmu.set_debug_trace_cart_writes(true);
    }

    let mut output: Vec<u8> = Vec::new();
    let mut cycles: u64 = 0;
    let start = Instant::now();
    let mut last_progress = Instant::now();
    let mut status: Option<Status> = None;
    let mut tail: VecDeque<(u16, u8, bool)> = VecDeque::with_capacity(options.dump_tail.max(1));
    let mut watch_value = options.watch.map(|addr| system.mmu.read_byte(addr));
    let mut watch_events = 0usize;
    let mut mem_message: Option<String> = None;
    let mut mem_started = false;
    let mut control_events = 0usize;
    let mut control_state = options.control_trace.then(|| {
        (
            system.mmu.read_byte(0xFF84),
            system.mmu.read_byte(0xFF85),
            system.mmu.read_byte(0xA000),
            system.mmu.rom_bank(),
            system.mmu.ram_bank,
            (0x0000..=0xBFFF).contains(&system.cpu.pc),
        )
    });

    while status.is_none() {
        let pc_before = system.cpu.pc;
        let opcode = system.mmu.read_byte(pc_before);
        let consumed = u64::from(system.step_instruction());
        cycles += consumed;
        if options.dump_tail > 0 {
            if tail.len() == options.dump_tail {
                tail.pop_front();
            }
            tail.push_back((pc_before, opcode, system.cpu.halted));
        }
        output.extend(system.mmu.serial.take_tx());

        if options.control_trace {
            for (addr, value) in system.mmu.take_debug_cart_writes() {
                if control_events >= 128 {
                    break;
                }
                control_events += 1;
                let printable = if value.is_ascii_graphic() || value == b' ' {
                    format!(" '{}'", value as char)
                } else {
                    String::new()
                };
                let mut err = std::io::stderr().lock();
                let _ = writeln!(
                    err,
                    "  control#{control_events}: cycles={cycles} PC={pc_before:04X}->{:04X} op={opcode:02X} cart_write={addr:04X}={value:02X}{printable} bank={}/{} A000={:02X}",
                    system.cpu.pc,
                    system.mmu.rom_bank(),
                    system.mmu.ram_bank,
                    system.mmu.read_byte(0xA000),
                );
                let _ = err.flush();
            }
        }

        if let Some(previous) = control_state {
            let current = (
                system.mmu.read_byte(0xFF84),
                system.mmu.read_byte(0xFF85),
                system.mmu.read_byte(0xA000),
                system.mmu.rom_bank(),
                system.mmu.ram_bank,
                (0x0000..=0xBFFF).contains(&system.cpu.pc),
            );
            if current != previous && control_events < 128 {
                control_events += 1;
                let sp = system.cpu.regs.sp();
                let stack: Vec<String> = (0..4u16)
                    .map(|offset| format!("{:02X}", system.mmu.read_byte(sp.wrapping_add(offset))))
                    .collect();
                let mut err = std::io::stderr().lock();
                let _ = writeln!(
                    err,
                    "  control#{control_events}: cycles={cycles} PC={pc_before:04X}->{:04X} op={opcode:02X} SP={sp:04X} AF={:04X} BC={:04X} DE={:04X} HL={:04X} stack={} hram84/85={:02X}/{:02X}->{:02X}/{:02X} A000={:02X}->{:02X} bank={}/{}->{}/{} exec={}->{}",
                    system.cpu.pc,
                    u16::from(system.cpu.regs.a) << 8 | u16::from(system.cpu.regs.f),
                    system.cpu.regs.bc(),
                    system.cpu.regs.de(),
                    system.cpu.regs.hl(),
                    stack.join(" "),
                    previous.0,
                    previous.1,
                    current.0,
                    current.1,
                    previous.2,
                    current.2,
                    previous.3,
                    previous.4,
                    current.3,
                    current.4,
                    previous.5,
                    current.5,
                );
                let _ = err.flush();
            }
            control_state = Some(current);
        }

        if let (Some(addr), Some(prev)) = (options.watch, watch_value) {
            let now = system.mmu.read_byte(addr);
            if now != prev {
                watch_value = Some(now);
                if watch_events < 40 {
                    watch_events += 1;
                    let mut err = std::io::stderr().lock();
                    let _ = writeln!(
                        err,
                        "  watch[{addr:04X}] {prev:02X}->{now:02X} after PC={pc_before:04X} op={opcode:02X} cycles={cycles} A={:02X}",
                        system.cpu.regs.a
                    );
                    if options.dump_tail > 0 {
                        for (tpc, top, thalt) in &tail {
                            let _ =
                                writeln!(err, "    trace PC={tpc:04X} op={top:02X} halted={thalt}");
                        }
                    }
                    let _ = err.flush();
                }
            }
        }

        // HALT is 0x76. STOP (0x10) also leaves the CPU in a halted state in
        // this core, but `--stop-on-halt` is specifically intended for ROMs
        // that execute HALT and must not stop on STOP's separate state.
        if stop_on_halt_requested(options.stop_on_halt, opcode, system.cpu.halted) {
            status = Some(Status::TimeoutCycles);
            break;
        }

        // Limits are independent safety boundaries. A completed instruction
        // may overshoot a boundary by its own cost, but the boundary wins over
        // protocol classification; never turn a late result into PASS.
        let wall = start.elapsed();
        if wall >= options.timeout {
            status = Some(Status::TimeoutWall);
            break;
        }
        if options.max_cycles != 0 && cycles >= options.max_cycles {
            status = Some(Status::TimeoutCycles);
            break;
        }

        if !output.is_empty() {
            if let Some(pass) = verdict(&output) {
                status = Some(if pass { Status::Pass } else { Status::Fail });
                break;
            }
        }

        // ROMs that do not print over the serial port report through the
        // documented `$A000` memory protocol instead. Require the `$80`
        // "running" marker before accepting a terminal code, because `$A000`
        // is zero until the ROM initialises it.
        if memory_signature_present(&system) {
            let code = system.mmu.read_byte(0xA000);
            if code == 0x80 {
                mem_started = true;
            } else if mem_started {
                let text = memory_text(&system);
                mem_message = Some(if text.trim().is_empty() {
                    if code == 0 {
                        "Passed".to_string()
                    } else {
                        format!("Failed #{code}")
                    }
                } else {
                    text.trim_end().to_string()
                });
                status = Some(if code == 0 {
                    Status::Pass
                } else {
                    Status::Fail
                });
                break;
            }
        }

        let wall = start.elapsed();
        if wall >= options.timeout {
            status = Some(Status::TimeoutWall);
            break;
        }
        if options.max_cycles != 0 && cycles >= options.max_cycles {
            status = Some(Status::TimeoutCycles);
            break;
        }

        // Periodic progress: 1 Hz, written to stderr and flushed.
        if !options.quiet && last_progress.elapsed() >= Duration::from_secs(1) {
            last_progress = Instant::now();
            let speed = cycles as f64 / wall.as_secs_f64().max(0.001) / 1e6;
            let mut err = std::io::stderr().lock();
            let _ = writeln!(
                err,
                "[{:>5.1}s] {:<28} cycles={:>12} {:>6.2} MHz PC={:04X} SP={:04X} halt={} ime={} IF={:02X} IE={:02X} last={:?}",
                wall.as_secs_f64(),
                truncate(&name, 28),
                cycles,
                speed,
                system.cpu.pc,
                system.cpu.regs.sp(),
                system.cpu.halted,
                system.cpu.ime,
                system.mmu.read_byte(0xFF0F),
                system.mmu.read_byte(0xFFFF),
                truncate(&last_message(&output), 32)
            );
            let _ = err.flush();
        }
    }

    if matches!(
        status,
        Some(Status::TimeoutWall) | Some(Status::TimeoutCycles)
    ) {
        let mut err = std::io::stderr().lock();
        if options.dump_tail > 0 {
            let _ = writeln!(err, "--- last {} instructions for {} ---", tail.len(), name);
            for (pc, opcode, halted) in &tail {
                let _ = writeln!(err, "  PC={pc:04X} op={opcode:02X} halted={halted}");
            }
        }
        let pc = system.cpu.pc;
        let _ = writeln!(
            err,
            "  final: PC={:04X} SP={:04X} A={:02X} F={:02X} BC={:04X} DE={:04X} HL={:04X} IME={} halted={}",
            pc,
            system.cpu.regs.sp(),
            system.cpu.regs.a,
            system.cpu.regs.f,
            system.cpu.regs.bc(),
            system.cpu.regs.de(),
            system.cpu.regs.hl(),
            system.cpu.ime,
            system.cpu.halted
        );
        let _ = writeln!(
            err,
            "  IF={:02X} IE={:02X} gb_id@D800={:02X}",
            system.mmu.read_byte(0xFF0F),
            system.mmu.read_byte(0xFFFF),
            system.mmu.read_byte(0xD800)
        );
        let sp = system.cpu.regs.sp();
        let stack: Vec<String> = (0..8u16)
            .map(|offset| format!("{:02X}", system.mmu.read_byte(sp.wrapping_add(offset))))
            .collect();
        let _ = writeln!(err, "  stack@SP: {}", stack.join(" "));
        let _ = writeln!(
            err,
            "  timer: DIV={:02X} TIMA={:02X} TMA={:02X} TAC={:02X}; apu: NR52={:02X} NR50={:02X} NR51={:02X}",
            system.mmu.read_byte(0xFF04),
            system.mmu.read_byte(0xFF05),
            system.mmu.read_byte(0xFF06),
            system.mmu.read_byte(0xFF07),
            system.mmu.read_byte(0xFF26),
            system.mmu.read_byte(0xFF24),
            system.mmu.read_byte(0xFF25),
        );
        let mut code = String::new();
        for offset in 0..8u16 {
            if offset > 0 {
                code.push(' ');
            }
            code.push_str(&format!(
                "{:02X}",
                system.mmu.read_byte(pc.wrapping_add(offset))
            ));
        }
        let _ = writeln!(err, "  code@PC: {code}");
        if memory_signature_present(&system) {
            let status_byte = system.mmu.read_byte(0xA000);
            let text = memory_text(&system);
            let text_len = text.chars().count();
            let text_tail: String = text.chars().skip(text_len.saturating_sub(160)).collect();
            let _ = writeln!(
                err,
                "  memory_protocol: A000={status_byte:02X} text_len={text_len} text_head={:?} text_tail={:?}",
                truncate(&text, 120),
                text_tail
            );
            let bank_count = system.mmu.external_ram.len() / 0x2000;
            let mut banks = String::new();
            for bank in 0..bank_count {
                let base = bank * 0x2000;
                let signature = &system.mmu.external_ram[base + 1..base + 4];
                let end = system.mmu.external_ram[base + 4..base + 0x2000]
                    .iter()
                    .position(|byte| *byte == 0)
                    .unwrap_or(0x1FFC);
                let bank_text =
                    String::from_utf8_lossy(&system.mmu.external_ram[base + 4..base + 4 + end]);
                banks.push_str(&format!(
                    " bank{}:status={:02X} sig={:02X}{:02X}{:02X} text={:?};",
                    bank,
                    system.mmu.external_ram[base],
                    signature[0],
                    signature[1],
                    signature[2],
                    truncate(&bank_text, 48)
                ));
            }
            let _ = writeln!(
                err,
                "  ram_banks: mode={} selected={} rom_bank={}{}",
                system.mmu.banking_mode,
                system.mmu.ram_bank,
                system.mmu.rom_bank(),
                banks
            );
        } else {
            let _ = writeln!(err, "  memory_protocol: signature absent");
        }
        let _ = err.flush();
    }

    let wall = start.elapsed();
    let message = match mem_message {
        Some(text) => text,
        None => String::from_utf8_lossy(&output).trim_end().to_string(),
    };

    if !options.quiet {
        let status = status.unwrap();
        let mut err = std::io::stderr().lock();
        let _ = writeln!(
            err,
            "[{:>5.1}s] {:<32} -> {} (cycles={})",
            wall.as_secs_f64(),
            truncate(&name, 32),
            status.label(),
            cycles
        );
        // Failure diagnostics are useless if only the last line is shown.
        if status == Status::Fail && !message.is_empty() {
            let _ = writeln!(err, "--- {name} output ---");
            let _ = writeln!(err, "{message}");
            let _ = writeln!(err, "--- end output ---");
        }
        let _ = err.flush();
    }

    Outcome {
        name,
        status: status.unwrap_or(Status::Error),
        cycles,
        wall,
        message,
    }
}

fn collect_roms(path: &Path) -> Vec<PathBuf> {
    if path.is_file() {
        return vec![path.to_path_buf()];
    }
    let mut roms = Vec::new();
    collect_recursive(path, &mut roms);
    roms.sort();
    roms
}

fn collect_recursive(dir: &Path, out: &mut Vec<PathBuf>) {
    if dir.file_name().is_some_and(|name| name == "oam_bug") {
        return;
    }
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            if path.file_name().is_some_and(|name| name == "oam_bug") {
                continue;
            }
            collect_recursive(&path, out);
        } else if path
            .extension()
            .map(|e| e.eq_ignore_ascii_case("gb") || e.eq_ignore_ascii_case("gbc"))
            .unwrap_or(false)
        {
            out.push(path);
        }
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let mut target: Option<PathBuf> = None;
    let mut max_cycles: u64 = DEFAULT_MAX_CYCLES;
    let mut timeout_secs: Option<u64> = None;
    let mut quiet = false;
    let mut dump_tail = 0usize;
    let mut stop_on_halt = false;
    let mut watch: Option<u16> = None;
    let mut force_dmg = false;
    let mut control_trace = false;
    let mut complex = false;

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--max-cycles" => {
                i += 1;
                max_cycles = args
                    .get(i)
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(max_cycles);
            }
            "--timeout-secs" => {
                i += 1;
                timeout_secs = Some(
                    args.get(i)
                        .and_then(|s| s.parse().ok())
                        .unwrap_or(DEFAULT_TIMEOUT_SECS),
                );
            }
            "--quiet" => quiet = true,
            "--dump-tail" => {
                i += 1;
                dump_tail = args.get(i).and_then(|s| s.parse().ok()).unwrap_or(32);
            }
            "--stop-on-halt" => stop_on_halt = true,
            "--force-dmg" => force_dmg = true,
            "--complex" => complex = true,
            "--control-trace" => control_trace = true,
            "--watch" => {
                i += 1;
                watch = args.get(i).and_then(|s| {
                    let s = s.trim();
                    let s = s
                        .strip_prefix("0x")
                        .or_else(|| s.strip_prefix("0X"))
                        .unwrap_or(s);
                    u16::from_str_radix(s, 16).ok()
                });
            }
            other if !other.starts_with('-') => target = Some(PathBuf::from(other)),
            other => {
                eprintln!("unknown argument: {other}");
                std::process::exit(2);
            }
        }
        i += 1;
    }

    let Some(target) = target else {
        eprintln!(
            "usage: blargg-runner <rom-or-directory> [--timeout-secs N] [--max-cycles N] [--quiet] [--force-dmg] [--complex]"
        );
        std::process::exit(2);
    };
    if target
        .components()
        .any(|component| component.as_os_str() == "oam_bug")
    {
        eprintln!("oam_bug is excluded from the hardware compatibility campaign");
        std::process::exit(2);
    }

    let roms = collect_roms(&target);
    if roms.is_empty() {
        eprintln!("no .gb ROMs found at {}", target.display());
        std::process::exit(2);
    }

    let timeout = Duration::from_secs(timeout_secs.unwrap_or(if complex {
        COMPLEX_TIMEOUT_SECS
    } else {
        DEFAULT_TIMEOUT_SECS
    }));
    let mut outcomes = Vec::new();
    for rom in &roms {
        outcomes.push(run_rom(
            rom,
            &RunOptions {
                max_cycles,
                timeout,
                quiet,
                dump_tail,
                stop_on_halt,
                watch,
                force_dmg,
                control_trace,
            },
        ));
    }

    println!();
    println!(
        "{:<46} {:>16} {:>14} {:>8}  MESSAGE",
        "ROM", "RESULT", "CYCLES", "WALL"
    );
    println!("{}", "-".repeat(120));
    for outcome in &outcomes {
        let short = outcome.message.lines().last().unwrap_or("");
        println!(
            "{:<46} {:>16} {:>14} {:>7.1}s  {}",
            truncate(&outcome.name, 46),
            outcome.status.label(),
            outcome.cycles,
            outcome.wall.as_secs_f64(),
            truncate(short, 50)
        );
    }
    println!("{}", "-".repeat(120));

    let count = |status: Status| outcomes.iter().filter(|o| o.status == status).count();
    let pass = count(Status::Pass);
    let fail = count(Status::Fail);
    let timeout_wall = count(Status::TimeoutWall);
    let timeout_cycles = count(Status::TimeoutCycles);
    let error = count(Status::Error);
    println!(
        "Total {} | PASS {} | FAIL {} | TIMEOUT(time) {} | TIMEOUT(cycles) {} | ERROR {}",
        outcomes.len(),
        pass,
        fail,
        timeout_wall,
        timeout_cycles,
        error
    );

    if fail + timeout_wall + timeout_cycles + error > 0 {
        std::process::exit(1);
    }
}

fn truncate(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        text.to_string()
    } else {
        let mut out: String = text.chars().take(max.saturating_sub(1)).collect();
        out.push('…');
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_individual_pass() {
        assert_eq!(verdict(b"01-special\n\nPassed\n"), Some(true));
        assert_eq!(verdict(b"Done\n"), Some(true));
    }

    #[test]
    fn detects_individual_failures() {
        assert_eq!(verdict(b"Failed\n"), Some(false));
        assert_eq!(verdict(b"Failed #3\n"), Some(false));
        assert_eq!(verdict(b"Internal error\n"), Some(false));
    }

    #[test]
    fn detects_multi_rom_summaries() {
        assert_eq!(verdict(b"Passed all tests\n"), Some(true));
        assert_eq!(verdict(b"Failed 2 tests.\n"), Some(false));
    }

    #[test]
    fn incomplete_output_is_not_a_verdict() {
        assert_eq!(verdict(b"01-special\n"), None);
        assert_eq!(verdict(b""), None);
    }

    #[test]
    fn stop_on_halt_does_not_confuse_stop_with_halt() {
        assert!(stop_on_halt_requested(true, 0x76, true));
        assert!(!stop_on_halt_requested(true, 0x10, true));
        assert!(!stop_on_halt_requested(true, 0x76, false));
        assert!(!stop_on_halt_requested(false, 0x76, true));
    }

    #[test]
    fn force_dmg_is_explicit_for_dmg_specific_suites() {
        let mut rom = vec![0; 0x150];
        rom[0x0143] = 0x80;
        assert!(select_cgb_mode(&rom, false));
        assert!(!select_cgb_mode(&rom, true));
    }

    #[test]
    fn memory_protocol_requires_signature_and_running_marker() {
        let mut system = System::new(false);
        assert!(!memory_signature_present(&system));
        system.mmu.write_byte(0xA001, 0xDE);
        system.mmu.write_byte(0xA002, 0xB0);
        system.mmu.write_byte(0xA003, 0x61);
        assert!(memory_signature_present(&system));

        system.mmu.write_byte(0xA000, 0x80);
        assert_eq!(system.mmu.read_byte(0xA000), 0x80);
        system.mmu.write_byte(0xA000, 0x00);
        assert_eq!(memory_text(&system), "");
    }

    #[test]
    fn memory_text_reads_zero_terminated_string() {
        let mut system = System::new(false);
        system.mmu.write_byte(0xA004, b'O');
        system.mmu.write_byte(0xA005, b'K');
        system.mmu.write_byte(0xA006, 0);
        system.mmu.write_byte(0xA007, b'X');
        assert_eq!(memory_text(&system), "OK");
    }

    #[test]
    fn recursive_collection_excludes_oam_bug_directory() {
        let roms = collect_roms(Path::new("gb-test-roms-master/oam_bug"));
        assert!(roms.is_empty());
    }
}
