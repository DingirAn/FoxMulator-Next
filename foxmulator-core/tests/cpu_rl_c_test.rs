#[cfg(test)]
mod tests {
    use foxmulator_core::cpu::flags::Flags;
    use foxmulator_core::{cpu::Cpu, mmu::Mmu};

    #[test]
    fn cpu_rl_c_carry_and_zero() {
        let mut mmu = Mmu::new();
        let mut cpu = Cpu::new();
        cpu.reset();

        // RL C is CB-prefixed: 0xCB 0x11. Pan Docs / gbdev.io/gb-opcodes.
        cpu.regs.c = 0x80;
        cpu.regs.f = 0x00; // no carry, other flags cleared
        let original_sp = cpu.regs.sp();

        mmu.write_byte(0x0100, 0xCB);
        mmu.write_byte(0x0101, 0x11);
        let cycles = cpu.step(&mut mmu);

        assert_eq!(cycles, 8, "RL C should take 8 cycles");
        assert_eq!(cpu.pc, 0x0102, "PC should advance by 2");
        assert_eq!(cpu.regs.sp(), original_sp, "SP should remain unchanged");
        assert_eq!(cpu.regs.c, 0x00, "C should rotate to 0x00");
        // Flags: Z set, C set (carry from bit7), N cleared, H cleared
        assert_eq!(
            cpu.regs.f,
            Flags::Z | Flags::C,
            "Flags should be Z=1 and C=1"
        );
    }

    #[test]
    fn cpu_rl_c_uses_carry_in() {
        let mut mmu = Mmu::new();
        let mut cpu = Cpu::new();
        cpu.reset();

        cpu.regs.c = 0x01;
        cpu.regs.f = Flags::C;
        mmu.write_byte(0x0100, 0xCB);
        mmu.write_byte(0x0101, 0x11);
        let cycles = cpu.step(&mut mmu);

        assert_eq!(cycles, 8);
        assert_eq!(cpu.regs.c, 0x03, "carry-in must be shifted into bit 0");
        assert_eq!(cpu.regs.f, 0, "old carry-out was 0 and result is non-zero");
    }
}
