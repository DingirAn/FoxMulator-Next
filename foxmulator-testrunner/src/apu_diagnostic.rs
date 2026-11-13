//! Temporary APU channel diagnostic; it does not change emulator runtime state.
//!
//! It configures an isolated CGB System per channel, verifies NR52, prints RMS,
//! peak and a small waveform preview, and optionally plays each channel through
//! PulseAudio. Usage: `apu-diagnostic [--play]`.

use foxmulator_core::apu::*;
use foxmulator_core::system::System;
use std::io::Write;

const CYCLES: u32 = 4_194_304 / 2;

fn configure(system: &mut System, channel: u8) {
    let mmu = &mut system.mmu;
    mmu.write_byte(NR52, 0x80);
    mmu.write_byte(NR50, 0x77);
    mmu.write_byte(NR51, (1 << (channel - 1)) | (1 << (channel + 3)));
    match channel {
        1 => {
            mmu.write_byte(NR10, 0x16); // sweep period 1, rising, shift 6
            mmu.write_byte(NR11, 0x80);
            mmu.write_byte(NR12, 0xF0);
            mmu.write_byte(NR13, 0x40);
            mmu.write_byte(NR14, 0x87);
        }
        2 => {
            mmu.write_byte(NR21, 0x80);
            mmu.write_byte(NR22, 0xF0);
            mmu.write_byte(NR23, 0x80);
            mmu.write_byte(NR24, 0x86);
        }
        3 => {
            for offset in 0..16 {
                mmu.write_byte(
                    WAVE_RAM_START + offset,
                    if offset & 1 == 0 { 0xF0 } else { 0x0F },
                );
            }
            mmu.write_byte(NR30, 0x80);
            mmu.write_byte(NR31, 0x00);
            mmu.write_byte(NR32, 0x20);
            mmu.write_byte(NR33, 0x60);
            mmu.write_byte(NR34, 0x86);
        }
        4 => {
            mmu.write_byte(NR41, 0x00);
            mmu.write_byte(NR42, 0xF0);
            mmu.write_byte(NR43, 0x13);
            mmu.write_byte(NR44, 0x80);
        }
        _ => unreachable!(),
    }
}

fn main() {
    let play = std::env::args().any(|arg| arg == "--play");
    for channel in 1..=4 {
        let mut system = System::new(true);
        configure(&mut system, channel);
        let trigger_active = system.mmu.read_byte(NR52) & (1 << (channel - 1)) != 0;
        system.mmu.tick(CYCLES);
        let samples = system.take_audio_samples();
        let peak = samples
            .iter()
            .fold(0.0f32, |peak, sample| peak.max(sample.abs()));
        let rms = (samples.iter().map(|sample| sample * sample).sum::<f32>()
            / samples.len().max(1) as f32)
            .sqrt();
        let active_after = system.mmu.read_byte(NR52) & (1 << (channel - 1)) != 0;
        let preview = samples
            .iter()
            .step_by(2)
            .take(16)
            .map(|sample| format!("{sample:.3}"))
            .collect::<Vec<_>>()
            .join(",");
        println!(
            "CH{channel} trigger_active={trigger_active} active_after_500ms={active_after} stereo_samples={} peak={peak:.4} rms={rms:.4} preview=[{preview}]",
            samples.len()
        );
        if play {
            let mut child = std::process::Command::new("paplay")
                .args([
                    "--raw",
                    "--format=float32le",
                    "--rate=44100",
                    "--channels=2",
                    "--stream-name=FoxMulator APU diagnostic",
                ])
                .stdin(std::process::Stdio::piped())
                .spawn()
                .expect("paplay available");
            let input = child.stdin.as_mut().expect("paplay stdin");
            for sample in &samples {
                input.write_all(&sample.to_le_bytes()).expect("audio write");
            }
            drop(child.stdin.take());
            let _ = child.wait();
        }
    }
}
