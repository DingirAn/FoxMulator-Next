#[cfg(test)]
mod tests {
    use foxmulator_core::cpu::flags::Flags;
    use foxmulator_core::{cpu::Cpu, mmu::Mmu};

    #[test]
    fn cpu_rlca_ones_input() {
        let mut mmu = Mmu::new();
        let mut cpu = Cpu::new();
        cpu.reset();
        mmu.write_byte(0x0100, 0x07); // RLCA
        mmu.write_byte(0x0101, 0); // unused
        cpu.regs.a = 0x01;
        let cycles = cpu.step(&mut mmu);
        assert_eq!(cycles, 4, "RLCA should consume 4 cycles");
        assert_eq!(cpu.regs.a, 0x02, "Result should be 0x02");
        assert_eq!(cpu.regs.f & Flags::C, 0, "Carry flag should be 0");
    }

    #[test]
    fn cpu_rlca_carry_input() {
        let mut mmu = Mmu::new();
        let mut cpu = Cpu::new();
        cpu.reset();
        mmu.write_byte(0x0100, 0x07); // RLCA
        cpu.regs.a = 0x80; // most significant bit set
        let cycles = cpu.step(&mut mmu);
        assert_eq!(cycles, 4, "RLCA should consume 4 cycles");
        assert_eq!(cpu.regs.a, 0x01, "Result should be 0x01");
        assert_eq!(cpu.regs.f & Flags::C, Flags::C, "Carry flag should be set");
        // RLCA always clears Z, N and H.
        assert_eq!(cpu.regs.f & Flags::Z, 0, "Zero flag must not be set");
        assert_eq!(cpu.regs.f & Flags::N, 0);
        assert_eq!(cpu.regs.f & Flags::H, 0);
    }
}
