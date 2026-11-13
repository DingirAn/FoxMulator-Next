#[cfg(test)]
mod tests {
    use foxmulator_core::{cpu::Cpu, mmu::Mmu};

    #[test]
    fn cpu_dec_c_half_carry_and_flags() {
        let mut mmu = Mmu::new();
        let mut cpu = Cpu::new();
        cpu.reset();

        // Prepare register C and flags (to test preservation of unaffected bits)
        cpu.regs.c = 0x10; // lower nibble 0, so half‑carry will be set
        cpu.regs.f = 0x00; // start with no flags
        let original_sp = cpu.regs.sp();

        // Execute opcode 0x0D at 0x0100
        mmu.write_byte(0x0100, 0x0D);
        let cycles = cpu.step(&mut mmu);

        // Validate execution
        assert_eq!(cycles, 4, "DEC C should take 4 cycles");
        assert_eq!(cpu.pc, 0x0101, "PC should advance to 0x0101");
        assert_eq!(cpu.regs.sp(), original_sp, "SP should remain unchanged");
        assert_eq!(cpu.regs.c, 0x0F, "C should decrement to 0x0F");

        // Flags: N set, H set, Z clear, C unaffected
        assert_eq!(cpu.regs.f & 0x40, 0x40, "N flag should be set");
        assert_eq!(cpu.regs.f & 0x20, 0x20, "Half-carry flag should be set");
        assert_eq!(cpu.regs.f & 0x80, 0x00, "Zero flag should be clear");
    }
}
