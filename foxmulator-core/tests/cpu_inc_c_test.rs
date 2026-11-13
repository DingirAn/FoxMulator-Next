#[cfg(test)]
mod tests {
    use foxmulator_core::{cpu::Cpu, mmu::Mmu};

    #[test]
    fn cpu_inc_c_updates_register_and_flags() {
        let mut mmu = Mmu::new();
        let mut cpu = Cpu::new();
        cpu.reset();

        // Set register C and flags
        cpu.regs.c = 0x12;
        cpu.regs.f = 0x01; // carry flag only
        let original_sp = cpu.regs.sp();

        // Write opcode 0x0C (INC C) at 0x0100
        mmu.write_byte(0x0100, 0x0C);
        let cycles = cpu.step(&mut mmu);

        // Validate core behavior
        assert_eq!(cycles, 4, "INC C should take 4 cycles");
        assert_eq!(cpu.pc, 0x0101, "PC should advance by 1");
        assert_eq!(cpu.regs.sp(), original_sp, "SP should remain unchanged");
        assert_eq!(cpu.regs.c, 0x13, "C should increment");

        // Flags: N cleared, H cleared (no half carry), Z cleared, carry unchanged
        assert_eq!(cpu.regs.f, 0x00, "Flags should be cleared 0 as default");
    }
}
