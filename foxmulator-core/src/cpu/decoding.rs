//! SM83 instruction decoder and executor.
//!
//! Reference: Pan Docs (GBDev) and <https://gbdev.io/gb-opcodes>.
//! Timings are expressed in T-cycles (4 T-cycles = 1 machine cycle).
//!
//! Flags: Z = 0x80, N = 0x40, H = 0x20, C = 0x10 (see `cpu::flags::Flags`).

use crate::cpu::alu;
use crate::cpu::flags::Flags;
use crate::cpu::registers::Registers;
use crate::mmu::Mmu;

pub type Cycles = u32;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn reg_get(idx: u8, regs: &Registers, mmu: &Mmu) -> u8 {
    match idx {
        0 => regs.b,
        1 => regs.c,
        2 => regs.d,
        3 => regs.e,
        4 => regs.h,
        5 => regs.l,
        6 => mmu.cpu_read_byte(regs.hl()),
        7 => regs.a,
        _ => unreachable!(),
    }
}

fn reg_set(idx: u8, val: u8, regs: &mut Registers, mmu: &mut Mmu) {
    match idx {
        0 => regs.b = val,
        1 => regs.c = val,
        2 => regs.d = val,
        3 => regs.e = val,
        4 => regs.h = val,
        5 => regs.l = val,
        6 => mmu.cpu_write_byte(regs.hl(), val),
        7 => regs.a = val,
        _ => unreachable!(),
    }
}

/// Fetch one byte at `*pc`, then advance PC.
fn fetch8(pc: &mut u16, mmu: &Mmu) -> u8 {
    let v = mmu.cpu_read_byte(*pc);
    *pc = pc.wrapping_add(1);
    v
}

/// Fetch a little-endian 16-bit immediate at `*pc`, then advance PC by two.
fn fetch16(pc: &mut u16, mmu: &Mmu) -> u16 {
    let lo = fetch8(pc, mmu);
    let hi = fetch8(pc, mmu);
    ((hi as u16) << 8) | lo as u16
}

/// Fetch a signed 8-bit immediate (e8) at `*pc`.
fn fetch_i8(pc: &mut u16, mmu: &Mmu) -> i8 {
    fetch8(pc, mmu) as i8
}

/// Push a 16-bit value: SP -= 2, then write low byte at SP and high at SP+1.
fn push16(regs: &mut Registers, mmu: &mut Mmu, val: u16) {
    let sp = regs.sp().wrapping_sub(2);
    regs.set_sp(sp);
    mmu.cpu_write_byte(sp, (val & 0xFF) as u8);
    mmu.cpu_write_byte(sp.wrapping_add(1), (val >> 8) as u8);
}

/// Pop a 16-bit value and advance SP by two.
fn pop16(regs: &mut Registers, mmu: &Mmu) -> u16 {
    let sp = regs.sp();
    let lo = mmu.cpu_read_byte(sp);
    let hi = mmu.cpu_read_byte(sp.wrapping_add(1));
    regs.set_sp(sp.wrapping_add(2));
    ((hi as u16) << 8) | lo as u16
}

#[inline]
fn flags8(z: bool, n: bool, h: bool, c: bool) -> u8 {
    (if z { Flags::Z } else { 0 })
        | (if n { Flags::N } else { 0 })
        | (if h { Flags::H } else { 0 })
        | (if c { Flags::C } else { 0 })
}

/// INC r: sets Z/H, clears N, preserves C.
fn inc8(regs: &mut Registers, val: u8) -> u8 {
    let res = val.wrapping_add(1);
    regs.f = (regs.f & Flags::C)
        | (if res == 0 { Flags::Z } else { 0 })
        | (if (val & 0x0F) == 0x0F { Flags::H } else { 0 });
    res
}

/// DEC r: sets Z/N/H, preserves C.
fn dec8(regs: &mut Registers, val: u8) -> u8 {
    let res = val.wrapping_sub(1);
    regs.f = (regs.f & Flags::C)
        | Flags::N
        | (if res == 0 { Flags::Z } else { 0 })
        | (if (val & 0x0F) == 0 { Flags::H } else { 0 });
    res
}

/// ADD HL,rr: Z preserved, N=0, H from bit 11, C from bit 15.
fn add_hl(regs: &mut Registers, val: u16) {
    let hl = regs.hl();
    let result = hl.wrapping_add(val);
    let half = ((hl & 0x0FFF) + (val & 0x0FFF)) > 0x0FFF;
    let carry = (hl as u32 + val as u32) > 0xFFFF;
    regs.f = (regs.f & Flags::Z)
        | (if half { Flags::H } else { 0 })
        | (if carry { Flags::C } else { 0 });
    regs.set_hl(result);
}

/// ADD SP,e8 / LD HL,SP+e8: Z=N=0, H/C from unsigned low-byte addition.
fn sp_plus_e8(regs: &mut Registers, e8: u8) -> u16 {
    let sp = regs.sp();
    let half = ((sp & 0x000F) + (e8 as u16 & 0x000F)) > 0x000F;
    let carry = ((sp & 0x00FF) + (e8 as u16 & 0x00FF)) > 0x00FF;
    regs.f = (if half { Flags::H } else { 0 }) | (if carry { Flags::C } else { 0 });
    sp.wrapping_add(e8 as i8 as i16 as u16)
}

/// ALU operation. `group` is the 3-bit operation selector (0..=7) used by the
/// 0x80-0xBF block and by the immediate ALU opcodes.
fn alu_op(group: u8, val: u8, regs: &mut Registers) {
    let a = regs.a;
    match group {
        // ADD A,r
        0 => {
            let r = alu::alu_add8(a as u16, val as u16);
            regs.a = r.result;
            regs.f = flags8(r.result == 0, false, r.half_carry, r.carry);
        }
        // ADC A,r
        1 => {
            let carry_in = (regs.f & Flags::C) != 0;
            let r = alu::alu_adc8(a as u16, val as u16, carry_in);
            regs.a = r.result;
            regs.f = flags8(r.result == 0, false, r.half_carry, r.carry);
        }
        // SUB r
        2 => {
            let r = alu::alu_sub8(a as u16, val as u16);
            regs.a = r.result;
            regs.f = flags8(r.result == 0, true, r.half_carry, r.carry);
        }
        // SBC A,r
        3 => {
            let carry_in = (regs.f & Flags::C) != 0;
            let r = alu::alu_sbc8(a as u16, val as u16, carry_in);
            regs.a = r.result;
            regs.f = flags8(r.result == 0, true, r.half_carry, r.carry);
        }
        // AND r
        4 => {
            let res = a & val;
            regs.a = res;
            regs.f = flags8(res == 0, false, true, false);
        }
        // XOR r
        5 => {
            let res = a ^ val;
            regs.a = res;
            regs.f = flags8(res == 0, false, false, false);
        }
        // OR r
        6 => {
            let res = a | val;
            regs.a = res;
            regs.f = flags8(res == 0, false, false, false);
        }
        // CP r
        7 => {
            let r = alu::alu_sub8(a as u16, val as u16);
            regs.f = flags8(r.result == 0, true, r.half_carry, r.carry);
        }
        _ => unreachable!(),
    }
}

// ---------------------------------------------------------------------------
// CB-prefixed instructions
// ---------------------------------------------------------------------------

fn exec_cb(sub: u8, regs: &mut Registers, mmu: &mut Mmu) -> Cycles {
    let idx = sub & 0x07;
    let bit = (sub >> 3) & 0x07;
    let val = reg_get(idx, regs, mmu);
    let is_hl = idx == 6;

    match sub >> 6 {
        // 0x00-0x3F: rotate / shift / swap
        0 => {
            let (res, carry) = match (sub >> 3) & 0x07 {
                // RLC
                0 => {
                    let c = (val >> 7) & 1;
                    ((val << 1) | c, c != 0)
                }
                // RRC
                1 => {
                    let c = val & 1;
                    ((val >> 1) | (c << 7), c != 0)
                }
                // RL
                2 => {
                    let c_in = (regs.f & Flags::C) != 0;
                    let c_out = (val & 0x80) != 0;
                    ((val << 1) | c_in as u8, c_out)
                }
                // RR
                3 => {
                    let c_in = (regs.f & Flags::C) != 0;
                    let c_out = (val & 0x01) != 0;
                    ((val >> 1) | ((c_in as u8) << 7), c_out)
                }
                // SLA
                4 => {
                    let c_out = (val & 0x80) != 0;
                    (val << 1, c_out)
                }
                // SRA (arithmetic, bit 7 preserved)
                5 => {
                    let c_out = (val & 0x01) != 0;
                    ((val >> 1) | (val & 0x80), c_out)
                }
                // SWAP
                6 => (val.rotate_right(4), false),
                // SRL (logical)
                7 => {
                    let c_out = (val & 0x01) != 0;
                    (val >> 1, c_out)
                }
                _ => unreachable!(),
            };
            reg_set(idx, res, regs, mmu);
            regs.f = flags8(res == 0, false, false, carry);
            if is_hl {
                16
            } else {
                8
            }
        }
        // 0x40-0x7F: BIT b,r
        1 => {
            let set = (val & (1 << bit)) != 0;
            regs.f = (regs.f & Flags::C) | Flags::H | (if set { 0 } else { Flags::Z });
            if is_hl {
                12
            } else {
                8
            }
        }
        // 0x80-0xBF: RES b,r
        2 => {
            reg_set(idx, val & !(1u8 << bit), regs, mmu);
            if is_hl {
                16
            } else {
                8
            }
        }
        // 0xC0-0xFF: SET b,r
        3 => {
            reg_set(idx, val | (1u8 << bit), regs, mmu);
            if is_hl {
                16
            } else {
                8
            }
        }
        _ => unreachable!(),
    }
}

// ---------------------------------------------------------------------------
// Primary instructions
// ---------------------------------------------------------------------------

pub fn exec(
    opcode: u8,
    regs: &mut Registers,
    pc: &mut u16,
    mmu: &mut Mmu,
    ime: &mut bool,
) -> Cycles {
    // 0xCB prefix: fetch the sub-opcode and dispatch.
    if opcode == 0xCB {
        *pc = pc.wrapping_add(1);
        let sub = fetch8(pc, mmu);
        return exec_cb(sub, regs, mmu);
    }

    // Consume the primary opcode byte.
    *pc = pc.wrapping_add(1);

    match opcode {
        // 0x00 NOP
        0x00 => 4,

        // 0x01 LD BC,d16
        0x01 => {
            let v = fetch16(pc, mmu);
            regs.set_bc(v);
            12
        }
        // 0x02 LD (BC),A
        0x02 => {
            mmu.cpu_write_byte(regs.bc(), regs.a);
            8
        }
        // 0x03 INC BC
        0x03 => {
            regs.set_bc(regs.bc().wrapping_add(1));
            8
        }
        // 0x04 INC B
        0x04 => {
            regs.b = inc8(regs, regs.b);
            4
        }
        // 0x05 DEC B
        0x05 => {
            regs.b = dec8(regs, regs.b);
            4
        }
        // 0x06 LD B,d8
        0x06 => {
            regs.b = fetch8(pc, mmu);
            8
        }
        // 0x07 RLCA
        0x07 => {
            let c = (regs.a >> 7) & 1;
            regs.a = (regs.a << 1) | c;
            regs.f = if c != 0 { Flags::C } else { 0 };
            4
        }
        // 0x08 LD (a16),SP
        0x08 => {
            let addr = fetch16(pc, mmu);
            let sp = regs.sp();
            mmu.cpu_write_byte(addr, (sp & 0xFF) as u8);
            mmu.cpu_write_byte(addr.wrapping_add(1), (sp >> 8) as u8);
            20
        }
        // 0x09 ADD HL,BC
        0x09 => {
            let bc = regs.bc();
            add_hl(regs, bc);
            8
        }
        // 0x0A LD A,(BC)
        0x0A => {
            regs.a = mmu.cpu_read_byte(regs.bc());
            8
        }
        // 0x0B DEC BC
        0x0B => {
            regs.set_bc(regs.bc().wrapping_sub(1));
            8
        }
        // 0x0C INC C
        0x0C => {
            regs.c = inc8(regs, regs.c);
            4
        }
        // 0x0D DEC C
        0x0D => {
            regs.c = dec8(regs, regs.c);
            4
        }
        // 0x0E LD C,d8
        0x0E => {
            regs.c = fetch8(pc, mmu);
            8
        }
        // 0x0F RRCA
        0x0F => {
            let c = regs.a & 1;
            regs.a = (regs.a >> 1) | (c << 7);
            regs.f = if c != 0 { Flags::C } else { 0 };
            4
        }

        // 0x10 STOP (2-byte instruction: 0x10 0x00)
        0x10 => {
            let _ = fetch8(pc, mmu);
            4
        }

        // 0x11 LD DE,d16
        0x11 => {
            let v = fetch16(pc, mmu);
            regs.set_de(v);
            12
        }
        // 0x12 LD (DE),A
        0x12 => {
            mmu.cpu_write_byte(regs.de(), regs.a);
            8
        }
        // 0x13 INC DE
        0x13 => {
            regs.set_de(regs.de().wrapping_add(1));
            8
        }
        // 0x14 INC D
        0x14 => {
            regs.d = inc8(regs, regs.d);
            4
        }
        // 0x15 DEC D
        0x15 => {
            regs.d = dec8(regs, regs.d);
            4
        }
        // 0x16 LD D,d8
        0x16 => {
            regs.d = fetch8(pc, mmu);
            8
        }
        // 0x17 RLA
        0x17 => {
            let c_in = (regs.f & Flags::C) != 0;
            let c_out = (regs.a & 0x80) != 0;
            regs.a = (regs.a << 1) | c_in as u8;
            regs.f = if c_out { Flags::C } else { 0 };
            4
        }
        // 0x18 JR e8
        0x18 => {
            let e = fetch_i8(pc, mmu);
            *pc = pc.wrapping_add(e as i16 as u16);
            12
        }
        // 0x19 ADD HL,DE
        0x19 => {
            let de = regs.de();
            add_hl(regs, de);
            8
        }
        // 0x1A LD A,(DE)
        0x1A => {
            regs.a = mmu.cpu_read_byte(regs.de());
            8
        }
        // 0x1B DEC DE
        0x1B => {
            regs.set_de(regs.de().wrapping_sub(1));
            8
        }
        // 0x1C INC E
        0x1C => {
            regs.e = inc8(regs, regs.e);
            4
        }
        // 0x1D DEC E
        0x1D => {
            regs.e = dec8(regs, regs.e);
            4
        }
        // 0x1E LD E,d8
        0x1E => {
            regs.e = fetch8(pc, mmu);
            8
        }
        // 0x1F RRA
        0x1F => {
            let c_in = (regs.f & Flags::C) != 0;
            let c_out = (regs.a & 0x01) != 0;
            regs.a = (regs.a >> 1) | ((c_in as u8) << 7);
            regs.f = if c_out { Flags::C } else { 0 };
            4
        }

        // 0x20 JR NZ,e8
        0x20 => {
            let e = fetch_i8(pc, mmu);
            if (regs.f & Flags::Z) == 0 {
                *pc = pc.wrapping_add(e as i16 as u16);
                12
            } else {
                8
            }
        }
        // 0x21 LD HL,d16
        0x21 => {
            let v = fetch16(pc, mmu);
            regs.set_hl(v);
            12
        }
        // 0x22 LD (HL+),A
        0x22 => {
            let addr = regs.hl();
            mmu.cpu_write_byte(addr, regs.a);
            regs.set_hl(addr.wrapping_add(1));
            8
        }
        // 0x23 INC HL
        0x23 => {
            regs.set_hl(regs.hl().wrapping_add(1));
            8
        }
        // 0x24 INC H
        0x24 => {
            regs.h = inc8(regs, regs.h);
            4
        }
        // 0x25 DEC H
        0x25 => {
            regs.h = dec8(regs, regs.h);
            4
        }
        // 0x26 LD H,d8
        0x26 => {
            regs.h = fetch8(pc, mmu);
            8
        }
        // 0x27 DAA
        0x27 => {
            let mut a = regs.a;
            let n = (regs.f & Flags::N) != 0;
            let h = (regs.f & Flags::H) != 0;
            let mut carry = (regs.f & Flags::C) != 0;

            if !n {
                if carry || a > 0x99 {
                    a = a.wrapping_add(0x60);
                    carry = true;
                }
                if h || (a & 0x0F) > 0x09 {
                    a = a.wrapping_add(0x06);
                }
            } else {
                if carry {
                    a = a.wrapping_sub(0x60);
                }
                if h {
                    a = a.wrapping_sub(0x06);
                }
            }

            regs.a = a;
            regs.f = (if a == 0 { Flags::Z } else { 0 })
                | (if n { Flags::N } else { 0 })
                | (if carry { Flags::C } else { 0 });
            4
        }
        // 0x28 JR Z,e8
        0x28 => {
            let e = fetch_i8(pc, mmu);
            if (regs.f & Flags::Z) != 0 {
                *pc = pc.wrapping_add(e as i16 as u16);
                12
            } else {
                8
            }
        }
        // 0x29 ADD HL,HL
        0x29 => {
            let hl = regs.hl();
            add_hl(regs, hl);
            8
        }
        // 0x2A LD A,(HL+)
        0x2A => {
            let addr = regs.hl();
            regs.a = mmu.cpu_read_byte(addr);
            regs.set_hl(addr.wrapping_add(1));
            8
        }
        // 0x2B DEC HL
        0x2B => {
            regs.set_hl(regs.hl().wrapping_sub(1));
            8
        }
        // 0x2C INC L
        0x2C => {
            regs.l = inc8(regs, regs.l);
            4
        }
        // 0x2D DEC L
        0x2D => {
            regs.l = dec8(regs, regs.l);
            4
        }
        // 0x2E LD L,d8
        0x2E => {
            regs.l = fetch8(pc, mmu);
            8
        }
        // 0x2F CPL
        0x2F => {
            regs.a = !regs.a;
            regs.f = (regs.f & (Flags::Z | Flags::C)) | Flags::N | Flags::H;
            4
        }

        // 0x30 JR NC,e8
        0x30 => {
            let e = fetch_i8(pc, mmu);
            if (regs.f & Flags::C) == 0 {
                *pc = pc.wrapping_add(e as i16 as u16);
                12
            } else {
                8
            }
        }
        // 0x31 LD SP,d16
        0x31 => {
            let v = fetch16(pc, mmu);
            regs.set_sp(v);
            12
        }
        // 0x32 LD (HL-),A
        0x32 => {
            let addr = regs.hl();
            mmu.cpu_write_byte(addr, regs.a);
            regs.set_hl(addr.wrapping_sub(1));
            8
        }
        // 0x33 INC SP
        0x33 => {
            regs.set_sp(regs.sp().wrapping_add(1));
            8
        }
        // 0x34 INC (HL)
        0x34 => {
            let addr = regs.hl();
            let val = mmu.cpu_read_byte(addr);
            let res = inc8(regs, val);
            mmu.cpu_write_byte(addr, res);
            12
        }
        // 0x35 DEC (HL)
        0x35 => {
            let addr = regs.hl();
            let val = mmu.cpu_read_byte(addr);
            let res = dec8(regs, val);
            mmu.cpu_write_byte(addr, res);
            12
        }
        // 0x36 LD (HL),d8
        0x36 => {
            let v = fetch8(pc, mmu);
            mmu.cpu_write_byte(regs.hl(), v);
            12
        }
        // 0x37 SCF
        0x37 => {
            regs.f = (regs.f & Flags::Z) | Flags::C;
            4
        }
        // 0x38 JR C,e8
        0x38 => {
            let e = fetch_i8(pc, mmu);
            if (regs.f & Flags::C) != 0 {
                *pc = pc.wrapping_add(e as i16 as u16);
                12
            } else {
                8
            }
        }
        // 0x39 ADD HL,SP
        0x39 => {
            let sp = regs.sp();
            add_hl(regs, sp);
            8
        }
        // 0x3A LD A,(HL-)
        0x3A => {
            let addr = regs.hl();
            regs.a = mmu.cpu_read_byte(addr);
            regs.set_hl(addr.wrapping_sub(1));
            8
        }
        // 0x3B DEC SP
        0x3B => {
            regs.set_sp(regs.sp().wrapping_sub(1));
            8
        }
        // 0x3C INC A
        0x3C => {
            regs.a = inc8(regs, regs.a);
            4
        }
        // 0x3D DEC A
        0x3D => {
            regs.a = dec8(regs, regs.a);
            4
        }
        // 0x3E LD A,d8
        0x3E => {
            regs.a = fetch8(pc, mmu);
            8
        }
        // 0x3F CCF
        0x3F => {
            let carry = regs.f & Flags::C;
            regs.f = (regs.f & Flags::Z) | (if carry != 0 { 0 } else { Flags::C });
            4
        }

        // 0x40-0x7F LD r,r' (0x76 = HALT)
        0x40..=0x7F => {
            if opcode == 0x76 {
                // HALT is handled by the CPU core; decoder treats it as a
                // 4-cycle instruction that leaves registers untouched.
                return 4;
            }
            let dst = (opcode >> 3) & 0x07;
            let src = opcode & 0x07;
            let val = reg_get(src, regs, mmu);
            reg_set(dst, val, regs, mmu);
            if src == 6 || dst == 6 {
                8
            } else {
                4
            }
        }

        // 0x80-0xBF ALU A,r
        0x80..=0xBF => {
            let group = (opcode >> 3) & 0x07;
            let idx = opcode & 0x07;
            let val = reg_get(idx, regs, mmu);
            alu_op(group, val, regs);
            if idx == 6 {
                8
            } else {
                4
            }
        }

        // 0xC0 RET NZ
        0xC0 => {
            if (regs.f & Flags::Z) == 0 {
                *pc = pop16(regs, mmu);
                20
            } else {
                8
            }
        }
        // 0xC1 POP BC
        0xC1 => {
            let v = pop16(regs, mmu);
            regs.set_bc(v);
            12
        }
        // 0xC2 JP NZ,a16
        0xC2 => {
            let addr = fetch16(pc, mmu);
            if (regs.f & Flags::Z) == 0 {
                *pc = addr;
                16
            } else {
                12
            }
        }
        // 0xC3 JP a16
        0xC3 => {
            *pc = fetch16(pc, mmu);
            16
        }
        // 0xC4 CALL NZ,a16
        0xC4 => {
            let addr = fetch16(pc, mmu);
            if (regs.f & Flags::Z) == 0 {
                push16(regs, mmu, *pc);
                *pc = addr;
                24
            } else {
                12
            }
        }
        // 0xC5 PUSH BC
        0xC5 => {
            let v = regs.bc();
            push16(regs, mmu, v);
            16
        }
        // 0xC6 ADD A,d8
        0xC6 => {
            let v = fetch8(pc, mmu);
            alu_op(0, v, regs);
            8
        }
        // 0xC7 RST 00h
        0xC7 => {
            push16(regs, mmu, *pc);
            *pc = 0x0000;
            16
        }
        // 0xC8 RET Z
        0xC8 => {
            if (regs.f & Flags::Z) != 0 {
                *pc = pop16(regs, mmu);
                20
            } else {
                8
            }
        }
        // 0xC9 RET
        0xC9 => {
            *pc = pop16(regs, mmu);
            16
        }
        // 0xCA JP Z,a16
        0xCA => {
            let addr = fetch16(pc, mmu);
            if (regs.f & Flags::Z) != 0 {
                *pc = addr;
                16
            } else {
                12
            }
        }
        // 0xCB is handled above; this arm is unreachable but keeps 0xCB out
        // of the catch-all if the prefix check is ever refactored.
        0xCB => unreachable!("CB prefix handled before the primary dispatch"),

        // 0xCC CALL Z,a16
        0xCC => {
            let addr = fetch16(pc, mmu);
            if (regs.f & Flags::Z) != 0 {
                push16(regs, mmu, *pc);
                *pc = addr;
                24
            } else {
                12
            }
        }
        // 0xCD CALL a16
        0xCD => {
            let addr = fetch16(pc, mmu);
            push16(regs, mmu, *pc);
            *pc = addr;
            24
        }
        // 0xCE ADC A,d8
        0xCE => {
            let v = fetch8(pc, mmu);
            alu_op(1, v, regs);
            8
        }
        // 0xCF RST 08h
        0xCF => {
            push16(regs, mmu, *pc);
            *pc = 0x0008;
            16
        }

        // 0xD0 RET NC
        0xD0 => {
            if (regs.f & Flags::C) == 0 {
                *pc = pop16(regs, mmu);
                20
            } else {
                8
            }
        }
        // 0xD1 POP DE
        0xD1 => {
            let v = pop16(regs, mmu);
            regs.set_de(v);
            12
        }
        // 0xD2 JP NC,a16
        0xD2 => {
            let addr = fetch16(pc, mmu);
            if (regs.f & Flags::C) == 0 {
                *pc = addr;
                16
            } else {
                12
            }
        }
        // Reserved opcodes are observable as illegal-instruction events on
        // reference cores, but they do not abort the emulator. The fetch has
        // already consumed the opcode, so model the instruction slot as an
        // otherwise inert 4-T-cycle operation.
        0xD3 => 4,
        // 0xD4 CALL NC,a16
        0xD4 => {
            let addr = fetch16(pc, mmu);
            if (regs.f & Flags::C) == 0 {
                push16(regs, mmu, *pc);
                *pc = addr;
                24
            } else {
                12
            }
        }
        // 0xD5 PUSH DE
        0xD5 => {
            let v = regs.de();
            push16(regs, mmu, v);
            16
        }
        // 0xD6 SUB d8
        0xD6 => {
            let v = fetch8(pc, mmu);
            alu_op(2, v, regs);
            8
        }
        // 0xD7 RST 10h
        0xD7 => {
            push16(regs, mmu, *pc);
            *pc = 0x0010;
            16
        }
        // 0xD8 RET C
        0xD8 => {
            if (regs.f & Flags::C) != 0 {
                *pc = pop16(regs, mmu);
                20
            } else {
                8
            }
        }
        // 0xD9 RETI
        0xD9 => {
            *pc = pop16(regs, mmu);
            *ime = true;
            16
        }
        // 0xDA JP C,a16
        0xDA => {
            let addr = fetch16(pc, mmu);
            if (regs.f & Flags::C) != 0 {
                *pc = addr;
                16
            } else {
                12
            }
        }
        // 0xDB reserved/illegal: inert 4-T-cycle instruction slot.
        0xDB => 4,
        // 0xDC CALL C,a16
        0xDC => {
            let addr = fetch16(pc, mmu);
            if (regs.f & Flags::C) != 0 {
                push16(regs, mmu, *pc);
                *pc = addr;
                24
            } else {
                12
            }
        }
        // 0xDD reserved/illegal: inert 4-T-cycle instruction slot.
        0xDD => 4,
        // 0xDE SBC A,d8
        0xDE => {
            let v = fetch8(pc, mmu);
            alu_op(3, v, regs);
            8
        }
        // 0xDF RST 18h
        0xDF => {
            push16(regs, mmu, *pc);
            *pc = 0x0018;
            16
        }

        // 0xE0 LDH (a8),A
        0xE0 => {
            let n = fetch8(pc, mmu);
            mmu.cpu_write_byte(0xFF00 | n as u16, regs.a);
            12
        }
        // 0xE1 POP HL
        0xE1 => {
            let v = pop16(regs, mmu);
            regs.set_hl(v);
            12
        }
        // 0xE2 LD (C),A
        0xE2 => {
            mmu.cpu_write_byte(0xFF00 | regs.c as u16, regs.a);
            8
        }
        // 0xE3/0xE4 reserved/illegal: inert 4-T-cycle instruction slots.
        0xE3 | 0xE4 => 4,
        // 0xE5 PUSH HL
        0xE5 => {
            let v = regs.hl();
            push16(regs, mmu, v);
            16
        }
        // 0xE6 AND d8
        0xE6 => {
            let v = fetch8(pc, mmu);
            alu_op(4, v, regs);
            8
        }
        // 0xE7 RST 20h
        0xE7 => {
            push16(regs, mmu, *pc);
            *pc = 0x0020;
            16
        }
        // 0xE8 ADD SP,e8
        0xE8 => {
            let e = fetch8(pc, mmu);
            let sp = sp_plus_e8(regs, e);
            regs.set_sp(sp);
            16
        }
        // 0xE9 JP (HL)
        0xE9 => {
            *pc = regs.hl();
            4
        }
        // 0xEA LD (a16),A
        0xEA => {
            let addr = fetch16(pc, mmu);
            mmu.cpu_write_byte(addr, regs.a);
            16
        }
        // 0xEB/0xEC/0xED reserved/illegal: inert 4-T-cycle instruction slots.
        0xEB..=0xED => 4,
        // 0xEE XOR d8
        0xEE => {
            let v = fetch8(pc, mmu);
            alu_op(5, v, regs);
            8
        }
        // 0xEF RST 28h
        0xEF => {
            push16(regs, mmu, *pc);
            *pc = 0x0028;
            16
        }

        // 0xF0 LDH A,(a8)
        0xF0 => {
            let n = fetch8(pc, mmu);
            regs.a = mmu.cpu_read_byte(0xFF00 | n as u16);
            12
        }
        // 0xF1 POP AF
        0xF1 => {
            let v = pop16(regs, mmu);
            regs.a = (v >> 8) as u8;
            regs.f = (v & 0xF0) as u8;
            12
        }
        // 0xF2 LD A,(C)
        0xF2 => {
            regs.a = mmu.cpu_read_byte(0xFF00 | regs.c as u16);
            8
        }
        // 0xF3 DI
        0xF3 => {
            *ime = false;
            4
        }
        // 0xF4 reserved/illegal: inert 4-T-cycle instruction slot.
        0xF4 => 4,
        // 0xF5 PUSH AF
        0xF5 => {
            // The lower nibble of F does not exist on real hardware.
            let v = ((regs.a as u16) << 8) | (regs.f & 0xF0) as u16;
            push16(regs, mmu, v);
            16
        }
        // 0xF6 OR d8
        0xF6 => {
            let v = fetch8(pc, mmu);
            alu_op(6, v, regs);
            8
        }
        // 0xF7 RST 30h
        0xF7 => {
            push16(regs, mmu, *pc);
            *pc = 0x0030;
            16
        }
        // 0xF8 LD HL,SP+e8
        0xF8 => {
            let e = fetch8(pc, mmu);
            let v = sp_plus_e8(regs, e);
            regs.set_hl(v);
            12
        }
        // 0xF9 LD SP,HL
        0xF9 => {
            regs.set_sp(regs.hl());
            8
        }
        // 0xFA LD A,(a16)
        0xFA => {
            let addr = fetch16(pc, mmu);
            regs.a = mmu.cpu_read_byte(addr);
            16
        }
        // 0xFB EI
        0xFB => {
            *ime = true;
            4
        }
        // 0xFC/0xFD reserved/illegal: inert 4-T-cycle instruction slots.
        0xFC | 0xFD => 4,
        // 0xFE CP d8
        0xFE => {
            let v = fetch8(pc, mmu);
            alu_op(7, v, regs);
            8
        }
        // 0xFF RST 38h
        0xFF => {
            push16(regs, mmu, *pc);
            *pc = 0x0038;
            16
        }
    }
}
