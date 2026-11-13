//! Independent SM83 opcode audit tests.
//!
//! Expected values in this file are derived from Pan Docs / gbdev.io/gb-opcodes
//! and from plain integer arithmetic, not from the decoder implementation.

#[cfg(test)]
mod cpu_opcode_audit {
    use foxmulator_core::cpu::flags::Flags;
    use foxmulator_core::{cpu::Cpu, mmu::Mmu};

    const START: u16 = 0x0100;

    fn new_cpu() -> (Cpu, Mmu) {
        let mut cpu = Cpu::new();
        cpu.reset();
        (cpu, Mmu::new())
    }

    fn load_at_start(mmu: &mut Mmu, bytes: &[u8]) {
        for (i, b) in bytes.iter().enumerate() {
            mmu.write_byte(START.wrapping_add(i as u16), *b);
        }
    }

    #[test]
    fn reserved_opcodes_lock_cpu_until_reset() {
        let (mut cpu, mut mmu) = new_cpu();
        cpu.regs.a = 0xA5;
        cpu.regs.f = 0xB0;
        for opcode in [
            0xD3, 0xDB, 0xDD, 0xE3, 0xE4, 0xEB, 0xEC, 0xED, 0xF4, 0xFC, 0xFD,
        ] {
            cpu.reset();
            cpu.regs.a = 0xA5;
            cpu.regs.f = 0xB0;
            load_at_start(&mut mmu, &[opcode, 0x3E, 0x42]);
            assert_eq!(cpu.step(&mut mmu), 4);
            assert!(cpu.locked);
            assert_eq!(cpu.pc, START + 1);
            assert_eq!(cpu.regs.a, 0xA5);
            assert_eq!(cpu.regs.f, 0xB0);

            mmu.write_byte(0xFFFF, 0x01);
            mmu.write_byte(0xFF0F, 0x01);
            cpu.ime = true;
            assert_eq!(cpu.step(&mut mmu), 4);
            assert!(cpu.locked);
            assert_eq!(cpu.pc, START + 1);
            assert_eq!(cpu.regs.a, 0xA5);

            cpu.reset();
            assert!(!cpu.locked);
        }
    }

    /// Size (in bytes) and T-cycles for opcodes that advance PC linearly.
    /// Returns `None` for control-flow opcodes and the CB prefix.
    fn expected_linear(op: u8) -> Option<(u16, u32)> {
        // Control-flow instructions (handled by dedicated tests below).
        const CONTROL: &[u8] = &[
            0x18, 0x20, 0x28, 0x30, 0x38, 0xC0, 0xC2, 0xC3, 0xC4, 0xC7, 0xC8, 0xC9, 0xCA, 0xCC,
            0xCD, 0xCF, 0xD0, 0xD2, 0xD4, 0xD7, 0xD8, 0xD9, 0xDA, 0xDC, 0xDF, 0xE7, 0xE9, 0xEF,
            0xF7, 0xFF,
        ];
        // Reserved/invalid opcodes are not part of the linear table.
        const INVALID: &[u8] = &[
            0xD3, 0xDB, 0xDD, 0xE3, 0xE4, 0xEB, 0xEC, 0xED, 0xF4, 0xFC, 0xFD,
        ];
        if CONTROL.contains(&op) || INVALID.contains(&op) || op == 0xCB {
            return None;
        }

        let size: u16 = match op {
            // 3-byte instructions.
            0x01 | 0x08 | 0x11 | 0x21 | 0x31 | 0xEA | 0xFA => 3,
            // 2-byte instructions.
            0x06 | 0x0E | 0x10 | 0x16 | 0x1E | 0x26 | 0x2E | 0x36 | 0x3E | 0xC6 | 0xCE | 0xD6
            | 0xDE | 0xE0 | 0xE6 | 0xE8 | 0xEE | 0xF0 | 0xF6 | 0xF8 | 0xFE => 2,
            _ => 1,
        };

        let cycles: u32 = match op {
            0x00 => 4,
            // 16-bit loads.
            0x01 | 0x11 | 0x21 | 0x31 => 12,
            0x08 => 20,
            0xEA | 0xFA => 16,
            // 16-bit INC/DEC.
            0x03 | 0x0B | 0x13 | 0x1B | 0x23 | 0x2B | 0x33 | 0x3B => 8,
            // ADD HL,rr.
            0x09 | 0x19 | 0x29 | 0x39 => 8,
            // 8-bit indirect loads.
            0x02 | 0x0A | 0x12 | 0x1A | 0x22 | 0x2A | 0x32 | 0x3A => 8,
            // INC/DEC (HL) and LD (HL),d8.
            0x34..=0x36 => 12,
            // INC/DEC r.
            0x04 | 0x05 | 0x0C | 0x0D | 0x14 | 0x15 | 0x1C | 0x1D | 0x24 | 0x25 | 0x2C | 0x2D
            | 0x3C | 0x3D => 4,
            // LD r,d8.
            0x06 | 0x0E | 0x16 | 0x1E | 0x26 | 0x2E | 0x3E => 8,
            // Rotates / misc.
            0x07 | 0x0F | 0x17 | 0x1F | 0x27 | 0x2F | 0x37 | 0x3F => 4,
            0x10 => 4,
            // LD r,r' block.
            0x40..=0x7F => {
                if op == 0x76 {
                    4
                } else if (op & 0x07) == 6 || ((op >> 3) & 0x07) == 6 {
                    8
                } else {
                    4
                }
            }
            // ALU A,r block.
            0x80..=0xBF => {
                if (op & 0x07) == 6 {
                    8
                } else {
                    4
                }
            }
            // ALU A,d8.
            0xC6 | 0xCE | 0xD6 | 0xDE | 0xE6 | 0xEE | 0xF6 | 0xFE => 8,
            // PUSH / POP.
            0xC5 | 0xD5 | 0xE5 | 0xF5 => 16,
            0xC1 | 0xD1 | 0xE1 | 0xF1 => 12,
            // LDH / LD (C),A.
            0xE0 | 0xF0 => 12,
            0xE2 | 0xF2 => 8,
            // SP arithmetic.
            0xE8 => 16,
            0xF8 => 12,
            0xF9 => 8,
            // DI / EI.
            0xF3 | 0xFB => 4,
            _ => panic!("missing cycle entry for opcode {op:#04X}"),
        };

        Some((size, cycles))
    }

    #[test]
    fn primary_linear_size_and_cycles() {
        for op in 0u8..=0xFF {
            let Some((size, expected_cycles)) = expected_linear(op) else {
                continue;
            };
            let (mut cpu, mut mmu) = new_cpu();
            // Safe distinct register values; HL points to writable RAM.
            cpu.regs.a = 0x12;
            cpu.regs.b = 0x34;
            cpu.regs.c = 0x56;
            cpu.regs.d = 0x78;
            cpu.regs.e = 0x9A;
            cpu.regs.set_hl(0xC000);
            cpu.regs.f = 0;
            load_at_start(&mut mmu, &[op, 0x00, 0x00]);

            let cycles = cpu.step(&mut mmu);

            assert_eq!(cycles, expected_cycles, "cycle count for opcode {op:#04X}");
            assert_eq!(
                cpu.pc,
                START.wrapping_add(size),
                "PC advance for opcode {op:#04X}"
            );
        }
    }

    // -----------------------------------------------------------------------
    // LD r,r' exhaustive
    // -----------------------------------------------------------------------

    fn reg_index_value(idx: u8, cpu: &Cpu, mmu: &Mmu) -> u8 {
        match idx {
            0 => cpu.regs.b,
            1 => cpu.regs.c,
            2 => cpu.regs.d,
            3 => cpu.regs.e,
            4 => cpu.regs.h,
            5 => cpu.regs.l,
            6 => mmu.read_byte(cpu.regs.hl()),
            7 => cpu.regs.a,
            _ => unreachable!(),
        }
    }

    #[test]
    fn ld_r_r_block_exhaustive() {
        for dst in 0u8..8 {
            for src in 0u8..8 {
                let op = 0x40 | (dst << 3) | src;
                if op == 0x76 {
                    continue;
                }
                let (mut cpu, mut mmu) = new_cpu();
                cpu.regs.b = 0x11;
                cpu.regs.c = 0x22;
                cpu.regs.d = 0x33;
                cpu.regs.e = 0x44;
                cpu.regs.h = 0x55;
                cpu.regs.l = 0x66;
                cpu.regs.a = 0x77;
                mmu.write_byte(cpu.regs.hl(), 0x88);
                let expected = reg_index_value(src, &cpu, &mmu);

                load_at_start(&mut mmu, &[op]);

                let cycles = cpu.step(&mut mmu);
                assert_eq!(
                    reg_index_value(dst, &cpu, &mmu),
                    expected,
                    "LD dst={dst} src={src} (op {op:#04X})"
                );
                let expected_cycles = if dst == 6 || src == 6 { 8 } else { 4 };
                assert_eq!(cycles, expected_cycles, "cycles for op {op:#04X}");
                assert_eq!(cpu.pc, START + 1);
                assert_eq!(cpu.regs.f, 0, "LD must not affect flags");
            }
        }
    }

    // -----------------------------------------------------------------------
    // ALU A,r exhaustive (64 combinations, with and without carry-in)
    // -----------------------------------------------------------------------

    fn expected_flags(z: bool, n: bool, h: bool, c: bool) -> u8 {
        (if z { Flags::Z } else { 0 })
            | (if n { Flags::N } else { 0 })
            | (if h { Flags::H } else { 0 })
            | (if c { Flags::C } else { 0 })
    }

    fn alu_expected(group: u8, a: u8, b: u8, carry_in: bool) -> (u8, u8) {
        let cin = carry_in as u16;
        match group {
            0 => {
                let sum = a as u16 + b as u16;
                let r = sum as u8;
                (
                    r,
                    expected_flags(r == 0, false, (a & 0x0F) + (b & 0x0F) > 0x0F, sum > 0xFF),
                )
            }
            1 => {
                let sum = a as u16 + b as u16 + cin;
                let r = sum as u8;
                (
                    r,
                    expected_flags(
                        r == 0,
                        false,
                        ((a & 0x0F) as u16 + (b & 0x0F) as u16 + cin) > 0x0F,
                        sum > 0xFF,
                    ),
                )
            }
            2 => {
                let r = a.wrapping_sub(b);
                (
                    r,
                    expected_flags(r == 0, true, (a & 0x0F) < (b & 0x0F), a < b),
                )
            }
            3 => {
                let sub = b as u16 + cin;
                let r = (a as u16).wrapping_sub(sub) as u8;
                (
                    r,
                    expected_flags(
                        r == 0,
                        true,
                        (a & 0x0F) < (b & 0x0F).wrapping_add(cin as u8),
                        (a as u16) < sub,
                    ),
                )
            }
            4 => {
                let r = a & b;
                (r, expected_flags(r == 0, false, true, false))
            }
            5 => {
                let r = a ^ b;
                (r, expected_flags(r == 0, false, false, false))
            }
            6 => {
                let r = a | b;
                (r, expected_flags(r == 0, false, false, false))
            }
            7 => {
                let r = a.wrapping_sub(b);
                (
                    a,
                    expected_flags(r == 0, true, (a & 0x0F) < (b & 0x0F), a < b),
                )
            }
            _ => unreachable!(),
        }
    }

    #[test]
    fn alu_a_r_block_exhaustive() {
        for carry_in in [false, true] {
            for group in 0u8..8 {
                for idx in 0u8..8 {
                    let op = 0x80 | (group << 3) | idx;
                    let (mut cpu, mut mmu) = new_cpu();
                    cpu.regs.a = 0x7F;
                    cpu.regs.b = 0x0F;
                    cpu.regs.c = 0x01;
                    cpu.regs.d = 0x80;
                    cpu.regs.e = 0xFF;
                    cpu.regs.h = 0x0F;
                    cpu.regs.l = 0x01;
                    cpu.regs.set_hl(0xC000);
                    mmu.write_byte(0xC000, 0x3C);
                    cpu.regs.f = if carry_in { Flags::C } else { 0 };

                    let a = cpu.regs.a;
                    let b = reg_index_value(idx, &cpu, &mmu);
                    let (expected_a, expected_f) = alu_expected(group, a, b, carry_in);

                    load_at_start(&mut mmu, &[op]);
                    let cycles = cpu.step(&mut mmu);

                    assert_eq!(
                        cpu.regs.a, expected_a,
                        "ALU group {group} idx {idx} carry_in {carry_in}"
                    );
                    assert_eq!(
                        cpu.regs.f, expected_f,
                        "flags for ALU group {group} idx {idx} carry_in {carry_in}"
                    );
                    assert_eq!(cycles, if idx == 6 { 8 } else { 4 });
                    assert_eq!(cpu.pc, START + 1);
                }
            }
        }
    }

    // -----------------------------------------------------------------------
    // CB-prefixed table
    // -----------------------------------------------------------------------

    fn rotate_expected(kind: u8, val: u8, carry_in: bool) -> (u8, bool) {
        match kind {
            0 => {
                let c = val >> 7;
                ((val << 1) | c, c != 0)
            }
            1 => {
                let c = val & 1;
                ((val >> 1) | (c << 7), c != 0)
            }
            2 => {
                let c_out = (val & 0x80) != 0;
                ((val << 1) | carry_in as u8, c_out)
            }
            3 => {
                let c_out = (val & 1) != 0;
                ((val >> 1) | ((carry_in as u8) << 7), c_out)
            }
            4 => {
                let c_out = (val & 0x80) != 0;
                (val << 1, c_out)
            }
            5 => {
                let c_out = (val & 1) != 0;
                ((val >> 1) | (val & 0x80), c_out)
            }
            6 => (val.rotate_right(4), false),
            7 => {
                let c_out = (val & 1) != 0;
                (val >> 1, c_out)
            }
            _ => unreachable!(),
        }
    }

    #[test]
    fn cb_table_exhaustive() {
        for sub in 0u8..=0xFF {
            let idx = sub & 0x07;
            let bit = (sub >> 3) & 0x07;
            let group = sub >> 6;

            let (mut cpu, mut mmu) = new_cpu();
            cpu.regs.b = 0x81;
            cpu.regs.c = 0x42;
            cpu.regs.d = 0x24;
            cpu.regs.e = 0x18;
            cpu.regs.h = 0x0F;
            cpu.regs.l = 0xF0;
            cpu.regs.a = 0x3C;
            cpu.regs.set_hl(0xC000);
            mmu.write_byte(0xC000, 0xA5);
            cpu.regs.f = Flags::C | Flags::Z;

            let old_val = reg_index_value(idx, &cpu, &mmu);
            let old_f = cpu.regs.f;

            load_at_start(&mut mmu, &[0xCB, sub]);
            let cycles = cpu.step(&mut mmu);

            let expected_cycles = match group {
                1 => {
                    if idx == 6 {
                        12
                    } else {
                        8
                    }
                }
                _ => {
                    if idx == 6 {
                        16
                    } else {
                        8
                    }
                }
            };
            assert_eq!(cycles, expected_cycles, "cycles for CB {sub:#04X}");
            assert_eq!(cpu.pc, START + 2, "PC after CB {sub:#04X}");

            match group {
                // Rotate / shift / swap.
                0 => {
                    let kind = (sub >> 3) & 0x07;
                    let (expected_val, expected_carry) =
                        rotate_expected(kind, old_val, (old_f & Flags::C) != 0);
                    let got_val = reg_index_value(idx, &cpu, &mmu);
                    assert_eq!(got_val, expected_val, "CB {sub:#04X} result");
                    let expected_f =
                        expected_flags(expected_val == 0, false, false, expected_carry);
                    assert_eq!(cpu.regs.f, expected_f, "CB {sub:#04X} flags");
                }
                // BIT b,r
                1 => {
                    let is_set = (old_val & (1 << bit)) != 0;
                    let expected_f =
                        (old_f & Flags::C) | Flags::H | (if is_set { 0 } else { Flags::Z });
                    assert_eq!(cpu.regs.f, expected_f, "BIT {sub:#04X} flags");
                    let got_val = reg_index_value(idx, &cpu, &mmu);
                    assert_eq!(got_val, old_val, "BIT must not modify its operand");
                }
                // RES b,r
                2 => {
                    let expected_val = old_val & !(1u8 << bit);
                    let got_val = reg_index_value(idx, &cpu, &mmu);
                    assert_eq!(got_val, expected_val, "RES {sub:#04X} result");
                    assert_eq!(cpu.regs.f, old_f, "RES must not affect flags");
                }
                // SET b,r
                3 => {
                    let expected_val = old_val | (1u8 << bit);
                    let got_val = reg_index_value(idx, &cpu, &mmu);
                    assert_eq!(got_val, expected_val, "SET {sub:#04X} result");
                    assert_eq!(cpu.regs.f, old_f, "SET must not affect flags");
                }
                _ => unreachable!(),
            }
        }
    }

    // -----------------------------------------------------------------------
    // Flag edge cases
    // -----------------------------------------------------------------------

    fn run_one(bytes: &[u8], setup: impl FnOnce(&mut Cpu, &mut Mmu)) -> (Cpu, Mmu) {
        let (mut cpu, mut mmu) = new_cpu();
        setup(&mut cpu, &mut mmu);
        load_at_start(&mut mmu, bytes);
        let _ = cpu.step(&mut mmu);
        (cpu, mmu)
    }

    #[test]
    fn add_adc_sub_sbc_cp_flags() {
        // ADD A,B: 0x0F + 0x01 -> 0x10, H=1.
        let (cpu, _) = run_one(&[0x80], |c, _| {
            c.regs.a = 0x0F;
            c.regs.b = 0x01;
        });
        assert_eq!(cpu.regs.a, 0x10);
        assert_eq!(cpu.regs.f, Flags::H);

        // ADD A,B: 0xFF + 0x01 -> 0x00, Z=H=C=1.
        let (cpu, _) = run_one(&[0x80], |c, _| {
            c.regs.a = 0xFF;
            c.regs.b = 0x01;
        });
        assert_eq!(cpu.regs.a, 0x00);
        assert_eq!(cpu.regs.f, Flags::Z | Flags::H | Flags::C);

        // ADC A,B with carry: 0xFF + 0 + 1 -> 0x00, Z=H=C=1.
        let (cpu, _) = run_one(&[0x88], |c, _| {
            c.regs.a = 0xFF;
            c.regs.b = 0x00;
            c.regs.f = Flags::C;
        });
        assert_eq!(cpu.regs.a, 0x00);
        assert_eq!(cpu.regs.f, Flags::Z | Flags::H | Flags::C);

        // SUB A,B: 0x00 - 0x01 -> 0xFF, N=H=C=1.
        let (cpu, _) = run_one(&[0x90], |c, _| {
            c.regs.a = 0x00;
            c.regs.b = 0x01;
        });
        assert_eq!(cpu.regs.a, 0xFF);
        assert_eq!(cpu.regs.f, Flags::N | Flags::H | Flags::C);

        // SBC A,B with carry: 0x00 - 0x00 - 1 -> 0xFF, N=H=C=1.
        let (cpu, _) = run_one(&[0x98], |c, _| {
            c.regs.a = 0x00;
            c.regs.b = 0x00;
            c.regs.f = Flags::C;
        });
        assert_eq!(cpu.regs.a, 0xFF);
        assert_eq!(cpu.regs.f, Flags::N | Flags::H | Flags::C);

        // CP A,B: A unchanged; 0x10-0x10 -> Z=N=1.
        let (cpu, _) = run_one(&[0xB8], |c, _| {
            c.regs.a = 0x10;
            c.regs.b = 0x10;
        });
        assert_eq!(cpu.regs.a, 0x10);
        assert_eq!(cpu.regs.f, Flags::Z | Flags::N);
    }

    #[test]
    fn logic_flags() {
        let (cpu, _) = run_one(&[0xA0], |c, _| {
            c.regs.a = 0xF0;
            c.regs.b = 0x0F;
        });
        assert_eq!(cpu.regs.a, 0x00);
        assert_eq!(cpu.regs.f, Flags::Z | Flags::H);

        let (cpu, _) = run_one(&[0xA8], |c, _| {
            c.regs.a = 0xFF;
            c.regs.b = 0xFF;
        });
        assert_eq!(cpu.regs.a, 0x00);
        assert_eq!(cpu.regs.f, Flags::Z);

        let (cpu, _) = run_one(&[0xB0], |c, _| {
            c.regs.a = 0x00;
            c.regs.b = 0x00;
        });
        assert_eq!(cpu.regs.a, 0x00);
        assert_eq!(cpu.regs.f, Flags::Z);
    }

    #[test]
    fn inc_dec_preserve_carry_and_set_half_carry() {
        let (cpu, _) = run_one(&[0x04], |c, _| {
            c.regs.b = 0x0F;
            c.regs.f = Flags::C;
        });
        assert_eq!(cpu.regs.b, 0x10);
        assert_eq!(cpu.regs.f, Flags::H | Flags::C);

        let (cpu, _) = run_one(&[0x04], |c, _| {
            c.regs.b = 0xFF;
            c.regs.f = Flags::C;
        });
        assert_eq!(cpu.regs.b, 0x00);
        assert_eq!(cpu.regs.f, Flags::Z | Flags::H | Flags::C);

        let (cpu, _) = run_one(&[0x05], |c, _| {
            c.regs.b = 0x10;
            c.regs.f = Flags::C;
        });
        assert_eq!(cpu.regs.b, 0x0F);
        assert_eq!(cpu.regs.f, Flags::N | Flags::H | Flags::C);

        let (cpu, _) = run_one(&[0x05], |c, _| {
            c.regs.b = 0x00;
            c.regs.f = Flags::C;
        });
        assert_eq!(cpu.regs.b, 0xFF);
        assert_eq!(cpu.regs.f, Flags::N | Flags::H | Flags::C);
    }

    #[test]
    fn add_hl_and_sp_offsets() {
        // ADD HL,BC: 0x0FFF + 0x0001 -> half-carry.
        let (cpu, _) = run_one(&[0x09], |c, _| {
            c.regs.set_hl(0x0FFF);
            c.regs.set_bc(0x0001);
            c.regs.f = Flags::Z;
        });
        assert_eq!(cpu.regs.hl(), 0x1000);
        assert_eq!(cpu.regs.f, Flags::Z | Flags::H);

        // ADD HL,BC: 0xFFFF + 0x0001 -> carry, Z preserved.
        let (cpu, _) = run_one(&[0x09], |c, _| {
            c.regs.set_hl(0xFFFF);
            c.regs.set_bc(0x0001);
            c.regs.f = Flags::Z;
        });
        assert_eq!(cpu.regs.hl(), 0x0000);
        assert_eq!(cpu.regs.f, Flags::Z | Flags::H | Flags::C);

        // ADD SP,e8: 0xFFF8 + 0x08 -> 0x0000, H and C set from low byte.
        let (cpu, _) = run_one(&[0xE8, 0x08], |c, _| {
            c.regs.set_sp(0xFFF8);
            c.regs.f = Flags::Z;
        });
        assert_eq!(cpu.regs.sp(), 0x0000);
        assert_eq!(cpu.regs.f, Flags::H | Flags::C);

        // LD HL,SP+e8: 0xFFF8 + 0x08 -> HL=0x0000, H/C set, Z cleared.
        let (cpu, _) = run_one(&[0xF8, 0x08], |c, _| {
            c.regs.set_sp(0xFFF8);
            c.regs.f = Flags::Z;
        });
        assert_eq!(cpu.regs.hl(), 0x0000);
        assert_eq!(cpu.regs.f, Flags::H | Flags::C);
    }

    #[test]
    fn scf_ccf_cpl() {
        let (cpu, _) = run_one(&[0x37], |c, _| c.regs.f = Flags::Z | Flags::N | Flags::H);
        assert_eq!(cpu.regs.f, Flags::Z | Flags::C);

        let (cpu, _) = run_one(&[0x3F], |c, _| c.regs.f = Flags::Z | Flags::N | Flags::H);
        assert_eq!(cpu.regs.f, Flags::Z | Flags::C);

        let (cpu, _) = run_one(&[0x3F], |c, _| c.regs.f = Flags::Z | Flags::C);
        assert_eq!(cpu.regs.f, Flags::Z);

        let (cpu, _) = run_one(&[0x2F], |c, _| {
            c.regs.a = 0x55;
            c.regs.f = Flags::Z | Flags::C;
        });
        assert_eq!(cpu.regs.a, 0xAA);
        assert_eq!(cpu.regs.f, Flags::Z | Flags::N | Flags::H | Flags::C);
    }

    #[test]
    fn daa_vectors() {
        // BCD 0x45 stays 0x45.
        let (cpu, _) = run_one(&[0x27], |c, _| c.regs.a = 0x45);
        assert_eq!(cpu.regs.a, 0x45);
        assert_eq!(cpu.regs.f, 0);

        // 0x0F -> 0x15 (low nibble adjustment), Z/A flags.
        let (cpu, _) = run_one(&[0x27], |c, _| c.regs.a = 0x0F);
        assert_eq!(cpu.regs.a, 0x15);
        assert_eq!(cpu.regs.f, 0);

        // 0x9A -> 0x00 with carry (0x9A + 0x60 + 0x06).
        let (cpu, _) = run_one(&[0x27], |c, _| c.regs.a = 0x9A);
        assert_eq!(cpu.regs.a, 0x00);
        assert_eq!(cpu.regs.f, Flags::Z | Flags::C);

        // After subtraction: 0x00 - 0x60 = 0xA0, C preserved.
        let (cpu, _) = run_one(&[0x27], |c, _| {
            c.regs.a = 0x00;
            c.regs.f = Flags::N | Flags::C;
        });
        assert_eq!(cpu.regs.a, 0xA0);
        assert_eq!(cpu.regs.f, Flags::N | Flags::C);

        // After subtraction with half-carry: 0x00 - 0x06 = 0xFA.
        let (cpu, _) = run_one(&[0x27], |c, _| {
            c.regs.a = 0x00;
            c.regs.f = Flags::N | Flags::H;
        });
        assert_eq!(cpu.regs.a, 0xFA);
        assert_eq!(cpu.regs.f, Flags::N);
    }

    // -----------------------------------------------------------------------
    // Control flow
    // -----------------------------------------------------------------------

    #[test]
    fn jp_conditional_cycles() {
        // JP NZ,a16 taken.
        let (cpu, _) = run_one(&[0xC2, 0x34, 0x12], |c, _| c.regs.f = 0);
        assert_eq!(cpu.pc, 0x1234);
        // Not taken.
        let (mut cpu, mut mmu) = new_cpu();
        cpu.regs.f = Flags::Z;
        load_at_start(&mut mmu, &[0xC2, 0x34, 0x12]);
        assert_eq!(cpu.step(&mut mmu), 12);
        assert_eq!(cpu.pc, START + 3);

        // JP a16 is always 16 cycles.
        let (mut cpu, mut mmu) = new_cpu();
        load_at_start(&mut mmu, &[0xC3, 0x34, 0x12]);
        assert_eq!(cpu.step(&mut mmu), 16);
        assert_eq!(cpu.pc, 0x1234);
    }

    #[test]
    fn call_and_ret_cycles_and_stack() {
        // CALL nn taken.
        let (mut cpu, mut mmu) = new_cpu();
        load_at_start(&mut mmu, &[0xCD, 0x00, 0x06]);
        assert_eq!(cpu.step(&mut mmu), 24);
        assert_eq!(cpu.pc, 0x0600);
        assert_eq!(cpu.regs.sp(), 0xFFFC);
        assert_eq!(mmu.read_byte(0xFFFC), 0x03);
        assert_eq!(mmu.read_byte(0xFFFD), 0x01);

        // RET.
        load_at_start(&mut mmu, &[0xC9]);
        cpu.pc = 0x0100;
        assert_eq!(cpu.step(&mut mmu), 16);
        assert_eq!(cpu.pc, 0x0103);
        assert_eq!(cpu.regs.sp(), 0xFFFE);

        // RET cc taken = 20, not taken = 8.
        let (mut cpu, mut mmu) = new_cpu();
        cpu.regs.set_sp(0xFFFC);
        mmu.write_byte(0xFFFC, 0x00);
        mmu.write_byte(0xFFFD, 0x05);
        cpu.regs.f = Flags::Z;
        load_at_start(&mut mmu, &[0xC8]);
        assert_eq!(cpu.step(&mut mmu), 20);
        assert_eq!(cpu.pc, 0x0500);

        let (mut cpu, mut mmu) = new_cpu();
        cpu.regs.f = 0;
        load_at_start(&mut mmu, &[0xC8]);
        assert_eq!(cpu.step(&mut mmu), 8);
        assert_eq!(cpu.pc, START + 1);
    }

    #[test]
    fn rst_vectors() {
        for (op, vector) in [
            (0xC7u8, 0x0000u16),
            (0xCF, 0x0008),
            (0xD7, 0x0010),
            (0xDF, 0x0018),
            (0xE7, 0x0020),
            (0xEF, 0x0028),
            (0xF7, 0x0030),
            (0xFF, 0x0038),
        ] {
            let (mut cpu, mut mmu) = new_cpu();
            load_at_start(&mut mmu, &[op]);
            assert_eq!(cpu.step(&mut mmu), 16, "RST {op:#04X} cycles");
            assert_eq!(cpu.pc, vector, "RST {op:#04X} vector");
            let sp = cpu.regs.sp();
            assert_eq!(mmu.read_byte(sp), 0x01, "return address low");
            assert_eq!(mmu.read_byte(sp + 1), 0x01, "return address high");
        }
    }

    #[test]
    fn jr_cycles() {
        let (mut cpu, mut mmu) = new_cpu();
        load_at_start(&mut mmu, &[0x18, 0x02]);
        assert_eq!(cpu.step(&mut mmu), 12);
        assert_eq!(cpu.pc, START + 2 + 2);

        let (mut cpu, mut mmu) = new_cpu();
        cpu.regs.f = Flags::Z;
        load_at_start(&mut mmu, &[0x20, 0x02]);
        assert_eq!(cpu.step(&mut mmu), 8);
        assert_eq!(cpu.pc, START + 2);
    }

    // -----------------------------------------------------------------------
    // HALT and interrupts
    // -----------------------------------------------------------------------

    #[test]
    fn halt_waits_for_enabled_interrupt() {
        let (mut cpu, mut mmu) = new_cpu();
        // IE and IF are plain MMU bytes in this core.
        mmu.write_byte(0xFFFF, 0x01); // VBlank enabled
        mmu.write_byte(0xFF0F, 0x00);
        load_at_start(&mut mmu, &[0x76, 0x00]);

        assert_eq!(cpu.step(&mut mmu), 4, "HALT consumes 4 cycles");
        assert!(cpu.halted);
        assert_eq!(cpu.pc, START + 1);

        // No pending interrupt: CPU stays halted.
        assert_eq!(cpu.step(&mut mmu), 4);
        assert_eq!(cpu.pc, START + 1);
        assert!(cpu.halted);

        // Request VBlank: CPU wakes and executes the next instruction.
        mmu.write_byte(0xFF0F, 0x01);
        let cycles = cpu.step(&mut mmu);
        assert_eq!(cycles, 4, "wake-up step executes the NOP");
        assert!(!cpu.halted);
        assert_eq!(cpu.pc, START + 2);
    }

    #[test]
    fn interrupt_service_vector_and_stack() {
        let (mut cpu, mut mmu) = new_cpu();
        cpu.ime = true;
        cpu.pc = 0x0100;
        cpu.regs.set_sp(0xFFFE);
        mmu.write_byte(0xFFFF, 0x05); // VBlank + Timer enabled
        mmu.write_byte(0xFF0F, 0x04); // Timer requested (lower priority)

        let cycles = cpu.step(&mut mmu);
        assert_eq!(cycles, 20);
        assert_eq!(cpu.pc, 0x0050, "Timer vector");
        assert!(!cpu.ime);
        assert_eq!(cpu.regs.sp(), 0xFFFC);
        assert_eq!(mmu.read_byte(0xFFFC), 0x00);
        assert_eq!(mmu.read_byte(0xFFFD), 0x01);
        assert_eq!(mmu.read_byte(0xFF0F) & 0x04, 0, "IF bit cleared");
    }

    #[test]
    fn interrupt_not_serviced_without_ime() {
        let (mut cpu, mut mmu) = new_cpu();
        cpu.ime = false;
        cpu.pc = 0x0100;
        mmu.write_byte(0xFFFF, 0x01);
        mmu.write_byte(0xFF0F, 0x01);
        load_at_start(&mut mmu, &[0x00]); // NOP

        assert_eq!(cpu.step(&mut mmu), 4);
        assert_eq!(cpu.pc, 0x0101, "NOP executed instead of interrupt");
        assert_eq!(mmu.read_byte(0xFF0F) & 0x01, 0x01, "IF unchanged");
    }
}
