#[cfg(test)]
mod tests {
    use foxmulator_core::cpu::flags::Flags;
    use foxmulator_core::{cpu::Cpu, mmu::Mmu};

    #[test]
    fn cpu_step_increments_pc() {
        let mut mmu = Mmu::new();
        let mut cpu = Cpu::new();
        cpu.reset();
        // NOP at 0x0100 increments PC by 1
        let cycles = cpu.step(&mut mmu);
        assert_eq!(cycles, 4, "NOP should take 4 cycles");
        assert_eq!(cpu.pc, 0x0101);
    }

    #[test]
    fn cpu_ld_a_b() {
        let mut mmu = Mmu::new();
        let mut cpu = Cpu::new();
        cpu.reset();
        mmu.write_byte(0x0100, 0x78);
        cpu.regs.b = 0x42;
        let cycles = cpu.step(&mut mmu);
        assert_eq!(cycles, 4);
        assert_eq!(cpu.regs.a, 0x42);
        assert_eq!(cpu.pc, 0x0101);
    }

    #[test]
    fn cpu_add_a_b() {
        let mut mmu = Mmu::new();
        let mut cpu = Cpu::new();
        cpu.reset();
        mmu.write_byte(0x0100, 0x80);
        cpu.regs.a = 0x10;
        cpu.regs.b = 0x20;
        let cycles = cpu.step(&mut mmu);
        assert_eq!(cycles, 4);
        assert_eq!(cpu.regs.a, 0x30);
        assert_eq!(cpu.regs.f, 0x00, "Flags should be cleared for 0x10+0x20");
        assert_eq!(cpu.pc, 0x0101);
    }

    #[test]
    fn cpu_ld_b_n() {
        let mut mmu = Mmu::new();
        let mut cpu = Cpu::new();
        cpu.reset();
        mmu.write_byte(0x0100, 0x06); // LD B,n
        mmu.write_byte(0x0101, 0xAB); // immediate value
        let cycles = cpu.step(&mut mmu);
        assert_eq!(cycles, 8);
        assert_eq!(cpu.regs.b, 0xAB);
        assert_eq!(cpu.pc, 0x0102);
    }

    #[test]
    fn cpu_ld_a_n() {
        let mut mmu = Mmu::new();
        let mut cpu = Cpu::new();
        cpu.reset();
        mmu.write_byte(0x0100, 0x3E); // LD A,n
        mmu.write_byte(0x0101, 0x55);
        let cycles = cpu.step(&mut mmu);
        assert_eq!(cycles, 8);
        assert_eq!(cpu.regs.a, 0x55);
        assert_eq!(cpu.pc, 0x0102);
    }

    #[test]
    fn cpu_add_a_b_carry() {
        let mut mmu = Mmu::new();
        let mut cpu = Cpu::new();
        cpu.reset();
        mmu.write_byte(0x0100, 0x80); // ADD A,B
        cpu.regs.a = 0xFF;
        cpu.regs.b = 0x02;
        let cycles = cpu.step(&mut mmu);
        assert_eq!(cycles, 4);
        assert_eq!(cpu.regs.a, 0x01);
        assert_eq!(cpu.regs.f & Flags::C, Flags::C, "Carry flag set");
    }

    #[test]
    fn cpu_add_a_b_half_carry() {
        let mut mmu = Mmu::new();
        let mut cpu = Cpu::new();
        cpu.reset();
        mmu.write_byte(0x0100, 0x80); // ADD A,B
        cpu.regs.a = 0x0F;
        cpu.regs.b = 0x01;
        let cycles = cpu.step(&mut mmu);
        assert_eq!(cycles, 4);
        assert_eq!(cpu.regs.a, 0x10);
        assert_eq!(cpu.regs.f & Flags::H, Flags::H, "Half carry flag set");
    }

    #[test]
    fn cpu_inc_b_zero() {
        let mut mmu = Mmu::new();
        let mut cpu = Cpu::new();
        cpu.reset();
        mmu.write_byte(0x0100, 0x04); // INC B
        cpu.regs.b = 0xFF;
        let cycles = cpu.step(&mut mmu);
        assert_eq!(cycles, 4);
        assert_eq!(cpu.regs.b, 0x00);
        assert_eq!(cpu.regs.f & Flags::Z, Flags::Z, "Zero flag set");
        assert_eq!(cpu.regs.f & Flags::H, Flags::H, "Half carry set");
        assert_eq!(cpu.regs.f & Flags::C, 0, "Carry cleared");
    }

    #[test]
    fn cpu_inc_b_half_carry() {
        let mut mmu = Mmu::new();
        let mut cpu = Cpu::new();
        cpu.reset();
        mmu.write_byte(0x0100, 0x04); // INC B
        cpu.regs.b = 0x0F;
        let cycles = cpu.step(&mut mmu);
        assert_eq!(cycles, 4);
        assert_eq!(cpu.regs.b, 0x10);
        assert_eq!(cpu.regs.f & Flags::H, Flags::H, "Half carry set");
    }

    #[test]
    fn cpu_ld_a_hl() {
        let mut mmu = Mmu::new();
        let mut cpu = Cpu::new();
        cpu.reset();
        mmu.write_byte(0x0100, 0x7E); // LD A,(HL)
        cpu.regs.h = 0x02;
        cpu.regs.l = 0x00;
        mmu.write_byte(0x0200, 0x42); // Place data at address 0x0200
        let cycles = cpu.step(&mut mmu);
        assert_eq!(cycles, 8);
        assert_eq!(cpu.regs.a, 0x42);
        assert_eq!(cpu.pc, 0x0101);
    }
}
