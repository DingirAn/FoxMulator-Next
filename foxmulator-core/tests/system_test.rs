//! End-to-end system loop tests: CPU + MMU + PPU + peripherals.

#[cfg(test)]
mod tests {
    use foxmulator_core::cpu::Cpu;
    use foxmulator_core::mmu::{Mmu, IE_ADDR, IF_ADDR, LCDC_ADDR, TAC_ADDR, TIMA_ADDR};
    use foxmulator_core::ppu::{Ppu, SCANLINE_CYCLES};
    use foxmulator_core::system::System;
    use foxmulator_core::EmuState;
    use std::path::PathBuf;

    fn state_path() -> PathBuf {
        let mut path = std::env::temp_dir();
        path.push(format!(
            "foxmulator-state-{}-{}.state",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        path
    }

    #[test]
    fn post_boot_dmg_register_state() {
        let mut cpu = Cpu::new();
        cpu.post_boot(false);

        assert_eq!(cpu.pc, 0x0100);
        assert_eq!(cpu.regs.sp(), 0xFFFE);
        assert_eq!(cpu.regs.a, 0x01);
        assert_eq!(cpu.regs.f, 0xB0);
        assert_eq!(cpu.regs.b, 0x00);
        assert_eq!(cpu.regs.c, 0x13);
        assert_eq!(cpu.regs.d, 0x00);
        assert_eq!(cpu.regs.e, 0xD8);
        assert_eq!(cpu.regs.h, 0x01);
        assert_eq!(cpu.regs.l, 0x4D);
        assert!(!cpu.ime);
        assert!(!cpu.halted);
    }

    #[test]
    fn post_boot_cgb_register_state() {
        let mut cpu = Cpu::new();
        cpu.post_boot(true);

        assert_eq!(cpu.regs.a, 0x11);
        assert_eq!(cpu.regs.f, 0x80);
        assert_eq!(cpu.regs.b, 0x00);
        assert_eq!(cpu.regs.c, 0x00);
        assert_eq!(cpu.regs.d, 0xFF);
        assert_eq!(cpu.regs.e, 0x56);
        assert_eq!(cpu.regs.h, 0x00);
        assert_eq!(cpu.regs.l, 0x0D);
        assert_eq!(cpu.pc, 0x0100);
    }

    #[test]
    fn cartridge_header_selects_hardware_before_system_initialization() {
        assert!(!System::cgb_mode_for_rom(&[0; 0x143]));
        for (header, expected_cgb) in [(0x00, false), (0x80, true), (0xC0, true)] {
            let mut rom = vec![0; 0x150];
            rom[0x0143] = header;
            let system = System::from_rom(rom);

            assert_eq!(system.cgb, expected_cgb, "header {header:02X}");
            assert_eq!(system.mmu.cgb, expected_cgb, "header {header:02X}");
            assert_eq!(
                system.cpu.regs.a,
                if expected_cgb { 0x11 } else { 0x01 },
                "post-boot CPU state must match header {header:02X}"
            );
            assert_eq!(
                system.mmu.read_byte(0xFF4F),
                if expected_cgb { 0x00 } else { 0xFF },
                "CGB VRAM register must be available only on CGB"
            );
        }
    }

    #[test]
    fn boot_rom_constructor_starts_before_cartridge_and_exposes_ff50() {
        let mut rom = vec![0u8; 0x8000];
        rom[0x143] = 0x80;
        rom[0x100] = 0x76;
        let system =
            System::from_rom_with_embedded_boot(rom).expect("valid integrated CGB boot ROM");
        assert_eq!(system.cpu.pc, 0);
        assert!(system.mmu.boot_rom_enabled);
        assert_eq!(system.mmu.read_byte(LCDC_ADDR) & 0x80, 0);

        let mut system = system;
        system.mmu.write_byte(0xFF50, 0x01);
        assert!(!system.mmu.boot_rom_enabled);
        assert_eq!(system.mmu.read_byte(0x0100), 0x76);
    }

    #[test]
    fn embedded_dmg_boot_rom_starts_and_unmaps_on_ff50() {
        let mut rom = vec![0u8; 0x8000];
        rom[0x100] = 0x76;
        let mut system =
            System::from_rom_with_embedded_boot(rom).expect("valid integrated DMG boot ROM");
        assert_eq!(system.cpu.pc, 0);
        assert!(system.mmu.boot_rom_enabled);
        system.mmu.write_byte(0xFF50, 0x01);
        assert!(!system.mmu.boot_rom_enabled);
        assert_eq!(system.mmu.read_byte(0x0100), 0x76);
    }

    #[test]
    fn system_respects_paused_state() {
        let mut system = System::new(false);
        system.state = EmuState::Paused;
        assert_eq!(system.step_instruction(), 0);
    }

    #[test]
    fn toggle_pause_roundtrips_running_state() {
        let mut system = System::new(false);
        assert!(system.toggle_pause());
        assert_eq!(system.state, EmuState::Paused);
        assert!(!system.toggle_pause());
        assert_eq!(system.state, EmuState::Running);
    }

    #[test]
    fn system_executes_nop_and_advances_pc() {
        let mut system = System::new(false);
        system.mmu.write_byte(0x0100, 0x00); // NOP
        system.mmu.write_byte(0x0101, 0x00); // NOP

        let cycles = system.step_instruction();
        assert_eq!(cycles, 4);
        assert_eq!(system.cpu.pc, 0x0101);
    }

    #[test]
    fn cgb_stop_switches_speed_when_key1_is_armed() {
        let mut cgb = System::new(true);
        cgb.mmu.write_byte(0x0100, 0x10); // STOP
        cgb.mmu.write_byte(0x0101, 0x00); // STOP padding byte
        cgb.mmu.key1_armed = true;

        assert_eq!(cgb.step_instruction(), 4);
        assert_eq!(cgb.cpu.pc, 0x0102);
        assert!(!cgb.cpu.halted);
        assert!(cgb.mmu.double_speed);
        assert_eq!(cgb.mmu.read_byte(0xFF4D), 0xFE);

        let mut dmg = System::new(false);
        dmg.mmu.write_byte(0x0100, 0x10);
        dmg.mmu.write_byte(0x0101, 0x00);
        assert_eq!(dmg.step_instruction(), 4);
        assert!(
            dmg.cpu.halted,
            "DMG STOP must halt instead of switching speed"
        );
    }

    #[test]
    fn cgb_reset_restores_default_object_priority_mode() {
        let mut system = System::new(true);
        system.mmu.opri = 1;
        system.mmu.double_speed = true;
        system.mmu.key1_armed = true;

        system.reset();

        assert_eq!(system.mmu.read_byte(0xFF6C), 0);
        assert_eq!(system.mmu.read_byte(0xFF4D), 0x7E);
    }

    #[test]
    fn system_advances_timer_with_instruction_cycles() {
        let mut system = System::new(false);
        system.mmu.write_byte(0x0100, 0x00); // NOP: 4 cycles

        // Run 64 NOPs = 256 T-cycles -> DIV increments once.
        for _ in 0..64 {
            system.step_instruction();
        }
        assert_eq!(system.mmu.read_byte(0xFF04), 1);
    }

    #[test]
    fn system_ppu_advances_one_line_with_cpu_clock() {
        let mut system = System::new(false);
        system.mmu.write_byte(LCDC_ADDR, 0x91);
        system.mmu.write_byte(0x0100, 0x00); // NOP
        for _ in 0..114 {
            system.step_instruction();
        }
        assert_eq!(system.mmu.read_byte(0xFF44), 1);
    }

    #[test]
    fn frame_completion_flag_is_set_after_full_frame() {
        let mut mmu = Mmu::new();
        mmu.write_byte(LCDC_ADDR, 0x91);
        let mut ppu = Ppu::new();
        ppu.reset(&mut mmu);
        assert!(!ppu.take_frame_ready());

        ppu.tick(SCANLINE_CYCLES * 154, &mut mmu);
        assert!(ppu.take_frame_ready(), "frame flag set on wrap");
        assert!(!ppu.take_frame_ready(), "consumed once");
    }

    #[test]
    fn system_step_frame_renders_and_returns_after_frame() {
        let mut system = System::new(false);
        system.mmu.write_byte(LCDC_ADDR, 0x91); // LCD on, BG on
                                                // Loop of NOPs at 0x0100, then JR back to 0x0100.
        system.mmu.write_byte(0x0100, 0x00);
        system.mmu.write_byte(0x0101, 0x18); // JR
        system.mmu.write_byte(0x0102, 0xFD); // -3 -> back to 0x0100

        let cycles = system.step_frame(200_000);
        assert!(cycles > 0);
        assert!(
            cycles <= 200_000,
            "step_frame stops when the frame ends or the cap is hit"
        );
        // A frame is 70224 T-cycles; the loop can overshoot by one instruction.
        assert!(
            (70_000..=71_000).contains(&cycles),
            "roughly one frame, got {cycles}"
        );
    }

    #[test]
    fn one_hardware_frame_budget_is_independent_of_cgb_cpu_speed() {
        for double_speed in [false, true] {
            let mut system = System::new(true);
            system.mmu.write_byte(LCDC_ADDR, 0x91);
            system.mmu.double_speed = double_speed;
            system.mmu.write_byte(0x0100, 0x00); // NOP

            let start = system.mmu.peripheral_ticks;
            let mut cpu_cycles = 0;
            while system.mmu.peripheral_ticks.wrapping_sub(start) < 70_224 {
                cpu_cycles += system.step_instruction();
            }

            assert!(system.ppu.take_frame_ready());
            if double_speed {
                assert!(cpu_cycles >= 140_448);
            } else {
                assert!(cpu_cycles >= 70_224);
            }
        }
    }

    #[test]
    fn save_state_roundtrip_restores_complete_system_state() {
        let path = state_path();
        let mut rom = vec![0u8; 0x8000];
        rom[0x0147] = 0x10; // MBC3 + RAM + RTC + battery.
        let mut system = System::new(true);
        system.load_cartridge(rom);
        system.mmu.ram_enabled = true;
        system.mmu.write_byte(0xC123, 0xA5);
        system.mmu.write_byte(0xFF4F, 1);
        system.mmu.write_byte(0x8000, 0x5A);
        system.mmu.write_byte(0xFF01, 0x77);
        system.mmu.double_speed = true;
        system.mmu.key1_armed = true;
        system.mmu.opri = 1;
        system.cpu.pc = 0x4567;
        system.cpu.regs.set_sp(0xD234);
        system.mmu.apu.set_cgb(true);
        system.save_state(&path).expect("state save succeeds");

        system.cpu.pc = 0x0100;
        system.cpu.regs.set_sp(0xFFFE);
        system.mmu.write_byte(0xC123, 0x00);
        system.mmu.write_byte(0xFF4F, 0);
        system.mmu.write_byte(0x8000, 0x00);
        system.mmu.write_byte(0xFF01, 0x00);
        system.load_state(&path).expect("state load succeeds");

        assert_eq!(system.cpu.pc, 0x4567);
        assert_eq!(system.cpu.regs.sp(), 0xD234);
        assert_eq!(system.mmu.read_byte(0xC123), 0xA5);
        assert_eq!(system.mmu.read_byte(0xFF4F), 1);
        assert_eq!(system.mmu.read_byte(0x8000), 0x5A);
        assert_eq!(system.mmu.read_byte(0xFF01), 0x77);
        assert_eq!(system.mmu.read_byte(0xFF4D), 0xFF);
        assert_eq!(system.mmu.read_byte(0xFF6C), 1);
        assert!(std::fs::read(&path).unwrap().starts_with(b"FOXSTATE1\n"));
        std::fs::remove_file(path).ok();
    }

    #[test]
    fn dual_systems_keep_cartridges_ram_and_states_independent() {
        fn rom(marker: u8) -> Vec<u8> {
            let mut data = vec![0u8; 0x8000];
            data[0x0147] = 0x03; // MBC1 + RAM + battery.
            data[0x0148] = 0x00;
            data[0x0149] = 0x02;
            data[0x4000] = marker;
            data
        }

        let left_state = state_path();
        let right_state = state_path();
        let mut left = System::from_rom(rom(0x11));
        let mut right = System::from_rom(rom(0x22));
        left.mmu.ram_enabled = true;
        right.mmu.ram_enabled = true;
        left.mmu.write_byte(0xA000, 0xA1);
        right.mmu.write_byte(0xA000, 0xB2);
        left.cpu.pc = 0x1234;
        right.cpu.pc = 0x5678;

        assert_eq!(left.mmu.read_byte(0x4000), 0x11);
        assert_eq!(right.mmu.read_byte(0x4000), 0x22);
        assert_eq!(left.mmu.read_byte(0xA000), 0xA1);
        assert_eq!(right.mmu.read_byte(0xA000), 0xB2);

        left.save_state(&left_state).expect("left state saves");
        right.save_state(&right_state).expect("right state saves");
        left.cpu.pc = 0;
        right.cpu.pc = 0;
        left.mmu.write_byte(0xA000, 0);
        right.mmu.write_byte(0xA000, 0);
        left.load_state(&left_state).expect("left state loads");
        assert_eq!(left.cpu.pc, 0x1234);
        assert_eq!(left.mmu.read_byte(0xA000), 0xA1);
        assert_eq!(right.cpu.pc, 0);
        assert_eq!(right.mmu.read_byte(0xA000), 0);

        left.load_cartridge(rom(0x33));
        assert_eq!(left.mmu.read_byte(0x4000), 0x33);
        assert_eq!(right.mmu.read_byte(0x4000), 0x22);
        std::fs::remove_file(left_state).ok();
        std::fs::remove_file(right_state).ok();
    }

    #[test]
    fn system_interrupt_dispatch_updates_pc_and_clears_if() {
        let mut system = System::new(false);
        system.cpu.ime = true;
        system.mmu.write_byte(IF_ADDR, 0x01); // VBlank pending
        system.mmu.write_byte(0xFFFF, 0x01); // VBlank enabled
        system.mmu.write_byte(0x0100, 0x00);

        let cycles = system.step_instruction();
        assert_eq!(cycles, 20, "interrupt dispatch costs 20 T-cycles");
        assert_eq!(system.cpu.pc, 0x0040);
        assert!(!system.cpu.ime);
        assert_eq!(system.mmu.read_byte(IF_ADDR) & 0x01, 0);
        // Return address pushed on the stack.
        let sp = system.cpu.regs.sp();
        assert_eq!(system.mmu.read_byte(sp), 0x00);
        assert_eq!(system.mmu.read_byte(sp + 1), 0x01);
    }

    #[test]
    fn system_joystick_input_reaches_mmu() {
        use foxmulator_core::joypad::Button;
        use foxmulator_core::mmu::JOYP_ADDR;

        let mut system = System::new(false);
        system.mmu.write_byte(JOYP_ADDR, 0x20); // select directions
        system.set_button(Button::Down, true);
        assert_eq!(system.mmu.read_byte(JOYP_ADDR) & 0x0F, 0b0111);
    }

    /// Regression for Blargg 02-interrupts (test 5, HALT).
    ///
    /// A halted CPU must keep consuming cycles so the timer keeps running, the
    /// timer overflow must request IF bit 2, and `(IF & IE & 0x1F) != 0` must
    /// wake the CPU even when IME is clear. Waking is not the same as servicing
    /// the interrupt: with IME clear the request stays pending.
    #[test]
    fn halt_wakes_on_timer_without_ime_and_keeps_request_pending() {
        let mut system = System::new(false);
        system.mmu.write_byte(IE_ADDR, 0x04);
        system.mmu.write_byte(IF_ADDR, 0x00);
        system.mmu.write_byte(TAC_ADDR, 0x05); // enabled, 16 T-cycle period
        system.mmu.write_byte(TIMA_ADDR, 0x00);
        system.mmu.write_byte(0x0100, 0x76); // HALT
        system.mmu.write_byte(0x0101, 0x00); // NOP after wake

        // Enter HALT.
        assert_eq!(system.step_instruction(), 4);
        assert!(system.cpu.halted);
        assert_eq!(system.cpu.pc, 0x0101);

        let mut saw_tima = false;
        let mut saw_if = false;
        let mut cycles = 0u32;
        for _ in 0..2000 {
            cycles += system.step_instruction();
            let tima = system.mmu.read_byte(TIMA_ADDR);
            if tima != 0 {
                saw_tima = true;
            }
            if system.mmu.read_byte(IF_ADDR) & 0x04 != 0 {
                saw_if = true;
            }
            if !system.cpu.halted {
                break;
            }
        }

        assert!(saw_tima, "TIMA must advance while the CPU is halted");
        assert!(
            saw_if,
            "TIMA overflow must raise IF bit 2 while the CPU is halted"
        );
        assert!(!system.cpu.halted, "timer request must wake the CPU");
        assert!(
            cycles >= 4096,
            "TIMA 0x00->overflow at TAC=0x05 needs at least 4096 T-cycles, got {cycles}"
        );
        // IME is clear: the request is pending but must not be serviced.
        assert!(!system.cpu.ime);
        assert_eq!(
            system.cpu.pc, 0x0102,
            "woke up and executed the NOP instead of servicing an interrupt"
        );
        assert_eq!(system.mmu.read_byte(IF_ADDR) & 0x04, 0x04);
    }

    /// With IME set, the same timer request both wakes and services the
    /// interrupt, jumping to the timer vector (0x0050) and clearing IME.
    #[test]
    fn halt_services_timer_interrupt_when_ime_enabled() {
        let mut system = System::new(false);
        system.mmu.write_byte(IE_ADDR, 0x04);
        system.mmu.write_byte(IF_ADDR, 0x00);
        system.mmu.write_byte(TAC_ADDR, 0x05);
        system.mmu.write_byte(TIMA_ADDR, 0x00);
        system.mmu.write_byte(0x0100, 0x76); // HALT
        system.mmu.write_byte(0x0101, 0x00);
        system.cpu.ime = true;

        // Enter HALT (no request pending yet).
        system.step_instruction();
        assert!(system.cpu.halted);

        let mut cycles = 0u32;
        for _ in 0..2000 {
            cycles += system.step_instruction();
            if !system.cpu.halted {
                break;
            }
        }

        assert!(!system.cpu.halted);
        assert!(!system.cpu.ime, "servicing an interrupt clears IME");
        assert_eq!(system.cpu.pc, 0x0050, "must jump to the timer vector");
        assert_eq!(system.mmu.read_byte(IF_ADDR) & 0x04, 0, "IF bit is cleared");
        assert!(cycles >= 4096);
    }
}
