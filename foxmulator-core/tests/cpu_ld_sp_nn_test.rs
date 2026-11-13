#[cfg(test)]
mod tests {
    use foxmulator_core::{cpu::Cpu, mmu::Mmu};

    #[test]
    fn cpu_ld_sp_to_mem() {
        let mut mmu = Mmu::new();
        let mut cpu = Cpu::new();
        cpu.reset();
        // Set SP to a known value
        cpu.regs.set_sp(0x1234);
        // Opcode 0x08 followed by little‑endian address 0x3412
        mmu.write_byte(0x0100, 0x08);
        mmu.write_byte(0x0101, 0x34);
        mmu.write_byte(0x0102, 0x12);
        let cycles = cpu.step(&mut mmu);
        assert_eq!(cycles, 20, "LD (nn),SP should take 20 cycles");
        assert_eq!(
            mmu.read_byte(0x1234),
            0x34,
            "Low byte of SP should be stored"
        );
        assert_eq!(
            mmu.read_byte(0x1235),
            0x12,
            "High byte of SP should be stored"
        );
        assert_eq!(cpu.pc, 0x0103, "PC should advance to 0x0103");
    }
}
