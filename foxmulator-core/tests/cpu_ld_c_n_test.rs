#[cfg(test)]
mod tests {
    use foxmulator_core::{cpu::Cpu, mmu::Mmu};

    #[test]
    fn cpu_ld_c_n() {
        let mut mmu = Mmu::new();
        let mut cpu = Cpu::new();
        cpu.reset();

        // Set a known flag state and SP
        cpu.regs.f = 0xAB;
        let original_sp = cpu.regs.sp();

        // Write opcode 0x0E (LD C,n) with immediate 0x42
        mmu.write_byte(0x0100, 0x0E);
        mmu.write_byte(0x0101, 0x42);
        let cycles = cpu.step(&mut mmu);

        assert_eq!(cycles, 8, "LD C,n should take 8 cycles");
        assert_eq!(cpu.pc, 0x0102, "PC should advance by 2");
        assert_eq!(cpu.regs.sp(), original_sp, "SP should stay unchanged");
        assert_eq!(cpu.regs.c, 0x42, "C should contain the immediate value");
        assert_eq!(cpu.regs.f, 0xAB, "Flags should be unchanged");
    }
}
