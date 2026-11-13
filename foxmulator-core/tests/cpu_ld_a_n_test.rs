#[cfg(test)]
mod tests {
    use foxmulator_core::{cpu::Cpu, mmu::Mmu};

    #[test]
    fn cpu_ld_a_n() {
        let mut mmu = Mmu::new();
        let mut cpu = Cpu::new();
        cpu.reset();
        // LD A,d8 is opcode 0x3E (0x03 is INC BC).
        mmu.write_byte(0x0100, 0x3E);
        mmu.write_byte(0x0101, 0xAF);
        let cycles = cpu.step(&mut mmu);
        assert_eq!(cycles, 8, "LD A,n should take 8 cycles");
        assert_eq!(cpu.regs.a, 0xAF, "A register should contain 0xAF");
        assert_eq!(cpu.pc, 0x0102, "PC should advance to 0x0102");
    }
}
