#[cfg(test)]
mod tests {
    use foxmulator_core::{cpu::Cpu, mmu::Mmu};

    #[test]
    fn cpu_dec_bc() {
        let mut mmu = Mmu::new();
        let mut cpu = Cpu::new();
        cpu.reset();
        // Set BC to a known value
        cpu.regs.set_bc(0x1234);
        // Set flags to a known value that should remain unchanged
        cpu.regs.f = 0xAB;
        // Opcode 0x0B
        mmu.write_byte(0x0100, 0x0B);
        let cycles = cpu.step(&mut mmu);
        assert_eq!(cycles, 8, "DEC BC should take 8 cycles");
        assert_eq!(cpu.regs.bc(), 0x1233, "BC should decrement by 1");
        assert_eq!(cpu.pc, 0x0101, "PC should advance to 0x0101");
        assert_eq!(cpu.regs.f, 0xAB, "Flags should remain unchanged");
    }
}
