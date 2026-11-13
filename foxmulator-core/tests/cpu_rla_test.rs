#[cfg(test)]
mod tests {
    use foxmulator_core::cpu::flags::Flags;
    use foxmulator_core::{cpu::Cpu, mmu::Mmu};

    #[test]
    fn cpu_rla_carry_set() {
        let mut mmu = Mmu::new();
        let mut cpu = Cpu::new();
        cpu.reset();
        cpu.regs.a = 0b11110000; // bit7=1
        cpu.regs.f = 0; // carry-in = 0
        mmu.write_byte(0x0100, 0x17); // RLA
        let cycles = cpu.step(&mut mmu);
        assert_eq!(cycles, 4, "RLA should take 4 cycles");
        assert_eq!(cpu.pc, 0x0101, "PC should advance by 1");
        // A = (A << 1) | old_carry = 0b11100000; new carry = old bit 7 = 1.
        assert_eq!(
            cpu.regs.a, 0b11100000,
            "A should rotate left with carry-in 0"
        );
        assert_eq!(cpu.regs.f & Flags::C, Flags::C, "Carry flag set");
    }

    #[test]
    fn cpu_rla_carry_clear() {
        let mut mmu = Mmu::new();
        let mut cpu = Cpu::new();
        cpu.reset();
        cpu.regs.a = 0b00101111; // bit7=0
        cpu.regs.f = 0;
        mmu.write_byte(0x0100, 0x17); // RLA
        let _cycles = cpu.step(&mut mmu);
        assert_eq!(cpu.regs.a, 0b01011110, "A should rotate left with no carry");
        assert_eq!(cpu.regs.f & Flags::C, 0, "Carry flag cleared");
    }

    #[test]
    fn cpu_rla_uses_carry_in() {
        let mut mmu = Mmu::new();
        let mut cpu = Cpu::new();
        cpu.reset();
        cpu.regs.a = 0b00000000;
        cpu.regs.f = Flags::C; // carry-in = 1
        mmu.write_byte(0x0100, 0x17); // RLA
        let cycles = cpu.step(&mut mmu);
        assert_eq!(cycles, 4);
        assert_eq!(cpu.regs.a, 0b00000001, "old carry must enter bit 0");
        assert_eq!(cpu.regs.f & Flags::C, 0, "old bit 7 was 0");
    }
}
