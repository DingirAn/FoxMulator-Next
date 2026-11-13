#[cfg(test)]
mod tests {
    use foxmulator_core::cpu::flags::Flags;
    use foxmulator_core::{cpu::Cpu, mmu::Mmu};

    #[test]
    fn cpu_ld_sp_nn() {
        let mut mmu = Mmu::new();
        let mut cpu = Cpu::new();
        cpu.reset();
        mmu.write_byte(0x0100, 0x31); // LD SP,nn
        mmu.write_byte(0x0101, 0x34);
        mmu.write_byte(0x0102, 0x12);
        let cycles = cpu.step(&mut mmu);
        assert_eq!(cycles, 12);
        assert_eq!(cpu.regs.sp(), 0x1234);
        assert_eq!(cpu.pc, 0x0103);
    }

    #[test]
    fn cpu_jr_c_taken() {
        let mut mmu = Mmu::new();
        let mut cpu = Cpu::new();
        cpu.reset();
        cpu.regs.f = Flags::C; // C flag set
        mmu.write_byte(0x0100, 0x38); // JR C,n
        mmu.write_byte(0x0101, 0x05); // offset
        let cycles = cpu.step(&mut mmu);
        assert_eq!(cycles, 12);
        assert_eq!(cpu.pc, 0x0100 + 2 + 5);
    }

    #[test]
    fn cpu_jr_c_not_taken() {
        let mut mmu = Mmu::new();
        let mut cpu = Cpu::new();
        cpu.reset();
        // C flag cleared
        mmu.write_byte(0x0100, 0x38); // JR C,n
        mmu.write_byte(0x0101, 0x05); // offset
        let cycles = cpu.step(&mut mmu);
        assert_eq!(cycles, 8);
        assert_eq!(cpu.pc, 0x0102);
    }

    #[test]
    fn cpu_add_hl_sp() {
        let mut mmu = Mmu::new();
        let mut cpu = Cpu::new();
        cpu.reset();
        cpu.regs.set_hl(0x2000);
        cpu.regs.set_sp(0x0300);
        mmu.write_byte(0x0100, 0x39); // ADD HL,SP
        let cycles = cpu.step(&mut mmu);
        assert_eq!(cycles, 8);
        assert_eq!(cpu.regs.hl(), 0x2300);
    }

    #[test]
    fn cpu_ld_a_hl_minus() {
        let mut mmu = Mmu::new();
        let mut cpu = Cpu::new();
        cpu.reset();
        cpu.regs.set_hl(0x3000);
        mmu.write_byte(0x3000, 0xAA);
        mmu.write_byte(0x0100, 0x3A); // LD A,(HL-)
        let cycles = cpu.step(&mut mmu);
        assert_eq!(cycles, 8);
        assert_eq!(cpu.regs.a, 0xAA);
        assert_eq!(cpu.regs.hl(), 0x2FFF);
    }

    #[test]
    fn cpu_dec_sp() {
        let mut mmu = Mmu::new();
        let mut cpu = Cpu::new();
        cpu.reset();
        cpu.regs.set_sp(0x1000);
        mmu.write_byte(0x0100, 0x3B); // DEC SP
        let cycles = cpu.step(&mut mmu);
        assert_eq!(cycles, 8);
        assert_eq!(cpu.regs.sp(), 0x0FFF);
    }

    #[test]
    fn cpu_inc_a_flags() {
        let mut mmu = Mmu::new();
        let mut cpu = Cpu::new();
        cpu.reset();
        cpu.regs.a = 0xFF; // will wrap to 0
        cpu.regs.f = Flags::C | Flags::H; // carry set and half carry set
        mmu.write_byte(0x0100, 0x3C); // INC A
        let cycles = cpu.step(&mut mmu);
        assert_eq!(cycles, 4);
        assert_eq!(cpu.regs.a, 0x00);
        // Zero flag set
        assert_eq!(cpu.regs.f & Flags::Z, Flags::Z);
        // Half carry should be set due to overflow
        assert_eq!(cpu.regs.f & Flags::H, Flags::H);
        // Carry should remain set
        assert_eq!(cpu.regs.f & Flags::C, Flags::C);
        // N flag cleared
        assert_eq!(cpu.regs.f & Flags::N, 0);
    }

    #[test]
    fn cpu_ccf_flips_carry() {
        let mut mmu = Mmu::new();
        let mut cpu = Cpu::new();
        cpu.reset();
        // Case: C flag set -> cleared
        cpu.regs.f = Flags::C;
        mmu.write_byte(0x0100, 0x3F); // CCF
        let cycles = cpu.step(&mut mmu);
        assert_eq!(cycles, 4);
        assert_eq!(cpu.regs.f & Flags::C, 0);
        assert_eq!(cpu.regs.f & Flags::N, 0);
        assert_eq!(cpu.regs.f & Flags::H, 0);

        // Case: C flag clear -> set
        cpu = Cpu::new();
        cpu.reset();
        cpu.regs.f = 0;
        mmu.write_byte(0x0100, 0x3F); // CCF
        let cycles = cpu.step(&mut mmu);
        assert_eq!(cycles, 4);
        assert_eq!(cpu.regs.f & Flags::C, Flags::C);
        assert_eq!(cpu.regs.f & Flags::N, 0);
        assert_eq!(cpu.regs.f & Flags::H, 0);
    }
}
