#[cfg(test)]
mod tests {
    use foxmulator_core::{cpu::Cpu, mmu::Mmu};

    #[test]
    fn cpu_ld_bc_nn() {
        let mut mmu = Mmu::new();
        let mut cpu = Cpu::new();
        cpu.reset();
        // Write opcode 0x01 and data 0x34,0x12 (little endian)
        mmu.write_byte(0x0100, 0x01);
        mmu.write_byte(0x0101, 0x34);
        mmu.write_byte(0x0102, 0x12);
        let cycles = cpu.step(&mut mmu);
        assert_eq!(cycles, 12, "LD BC,nn should take 12 cycles");
        assert_eq!(cpu.regs.bc(), 0x1234, "BC should be loaded with 0x1234");
        assert_eq!(cpu.pc, 0x0103, "PC should advance to 0x0103");
    }
}
