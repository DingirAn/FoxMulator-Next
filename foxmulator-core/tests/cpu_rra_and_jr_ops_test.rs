#[cfg(test)]
mod tests {
    use foxmulator_core::cpu::flags::Flags;
    use foxmulator_core::{cpu::Cpu, mmu::Mmu};

    #[test]
    fn cpu_rra_carry_and_zero() {
        let mut mmu = Mmu::new();
        let mut cpu = Cpu::new();
        cpu.reset();
        cpu.regs.a = 0b11000101; // bit7=1, bit0=1
        cpu.regs.f = Flags::C; // carry set (bit 4)
        mmu.write_byte(0x0100, 0x1F); // RRA
        let cycles = cpu.step(&mut mmu);
        assert_eq!(cycles, 4);
        // new A should be (0b11000101 >> 1) | (carry << 7) = 0b01100010 | 0x80 = 0b11100010
        assert_eq!(cpu.regs.a, 0b11100010);
        // carry should now be old bit0 = 1
        assert_eq!(cpu.regs.f & Flags::C, Flags::C);
        // Z flag should be cleared
        assert_eq!(cpu.regs.f & Flags::Z, 0x00);
    }

    #[test]
    fn cpu_jr_nz_taken() {
        let mut mmu = Mmu::new();
        let mut cpu = Cpu::new();
        cpu.reset();
        cpu.regs.f = 0x00; // Z flag clear
        mmu.write_byte(0x0100, 0x20); // JR NZ n
        mmu.write_byte(0x0101, 0x04); // offset +4
        let cycles = cpu.step(&mut mmu);
        assert_eq!(cycles, 12);
        // PC should jump to 0x0100 + 2 + 4 = 0x0106
        assert_eq!(cpu.pc, 0x0106);
    }

    #[test]
    fn cpu_jr_nz_not_taken() {
        let mut mmu = Mmu::new();
        let mut cpu = Cpu::new();
        cpu.reset();
        cpu.regs.f = 0x80; // Z flag set
        mmu.write_byte(0x0100, 0x20); // JR NZ n
        mmu.write_byte(0x0101, 0x04); // offset +4
        let cycles = cpu.step(&mut mmu);
        assert_eq!(cycles, 8);
        // PC should advance by 2 only
        assert_eq!(cpu.pc, 0x0102);
    }

    #[test]
    fn cpu_ld_hl_nn() {
        let mut mmu = Mmu::new();
        let mut cpu = Cpu::new();
        cpu.reset();
        mmu.write_byte(0x0100, 0x21); // LD HL,nn
        mmu.write_byte(0x0101, 0x34); // low
        mmu.write_byte(0x0102, 0x12); // high
        let cycles = cpu.step(&mut mmu);
        assert_eq!(cycles, 12);
        assert_eq!(cpu.regs.hl(), 0x1234);
        assert_eq!(cpu.pc, 0x0103);
    }

    #[test]
    fn cpu_ld_hl_plus_a() {
        let mut mmu = Mmu::new();
        let mut cpu = Cpu::new();
        cpu.reset();
        cpu.regs.a = 0x55;
        cpu.regs.set_hl(0x2000);
        mmu.write_byte(0x0100, 0x22); // LD (HL+),A
        let cycles = cpu.step(&mut mmu);
        assert_eq!(cycles, 8);
        assert_eq!(mmu.read_byte(0x2000), 0x55);
        assert_eq!(cpu.regs.hl(), 0x2001);
        assert_eq!(cpu.pc, 0x0101);
    }

    #[test]
    fn cpu_inc_hl() {
        let mut mmu = Mmu::new();
        let mut cpu = Cpu::new();
        cpu.reset();
        cpu.regs.set_hl(0x00FF);
        mmu.write_byte(0x0100, 0x23); // INC HL
        let cycles = cpu.step(&mut mmu);
        assert_eq!(cycles, 8);
        assert_eq!(cpu.regs.hl(), 0x0100);
        assert_eq!(cpu.pc, 0x0101);
    }

    #[test]
    fn cpu_jr_nc_taken() {
        let mut mmu = Mmu::new();
        let mut cpu = Cpu::new();
        cpu.reset();
        cpu.regs.f = 0x00; // Carry clear
        mmu.write_byte(0x0100, 0x30); // JR NC
        mmu.write_byte(0x0101, 0x05); // jump 5
        let cycles = cpu.step(&mut mmu);
        assert_eq!(cycles, 12, "JR NC should take 12 cycles");
        assert_eq!(cpu.pc, 0x0100 + 2 + 5, "PC should jump to 0x0107");
    }

    #[test]
    fn cpu_jr_nc_not_taken() {
        let mut mmu = Mmu::new();
        let mut cpu = Cpu::new();
        cpu.reset();
        cpu.regs.f = 0x10; // Carry set
        mmu.write_byte(0x0100, 0x30); // JR NC
        mmu.write_byte(0x0101, 0x05); // offset
        let cycles = cpu.step(&mut mmu);
        assert_eq!(cycles, 8);
        assert_eq!(cpu.pc, 0x0102);
    }

    #[test]
    fn cpu_scf_sets_carry() {
        let mut mmu = Mmu::new();
        let mut cpu = Cpu::new();
        cpu.reset();
        cpu.regs.f = 0x00; // No flags
        mmu.write_byte(0x0100, 0x37); // SCF
        let cycles = cpu.step(&mut mmu);
        assert_eq!(cycles, 4);
        // carry set
        assert_eq!(cpu.regs.f & 0x10, 0x10);
        // N and H cleared
        assert_eq!(cpu.regs.f & 0x40, 0); // N
        assert_eq!(cpu.regs.f & 0x20, 0); // H
                                          // Z unchanged
        assert_eq!(cpu.regs.f & 0x80, 0);
    }
}
