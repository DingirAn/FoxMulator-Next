#[cfg(test)]
mod tests {
    use foxmulator_core::{cpu::Cpu, mmu::Mmu};

    #[test]
    fn cpu_jr_z_taken_positive_offset() {
        let mut mmu = Mmu::new();
        let mut cpu = Cpu::new();
        cpu.reset();
        cpu.regs.f = 0x80; // Z flag set
        mmu.write_byte(0x0100, 0x28); // JR Z,n
        mmu.write_byte(0x0101, 0x03); // offset +3
        let cycles = cpu.step(&mut mmu);
        assert_eq!(cycles, 12);
        assert_eq!(
            cpu.pc,
            0x0100 + 3 + 2,
            "PC should jump past next byte plus offset"
        );
    }

    #[test]
    fn cpu_jr_z_taken_negative_offset() {
        let mut mmu = Mmu::new();
        let mut cpu = Cpu::new();
        cpu.reset();
        cpu.regs.f = 0x80; // Z flag set
        mmu.write_byte(0x0100, 0x28); // JR Z,n
        mmu.write_byte(0x0101, 0xFE); // offset -2
        let cycles = cpu.step(&mut mmu);
        assert_eq!(cycles, 12);
        assert_eq!(cpu.pc, 0x0100, "PC should return to start");
    }

    #[test]
    fn cpu_jr_z_not_taken() {
        let mut mmu = Mmu::new();
        let mut cpu = Cpu::new();
        cpu.reset();
        cpu.regs.f = 0x00; // Z flag clear
        mmu.write_byte(0x0100, 0x28); // JR Z,n
        mmu.write_byte(0x0101, 0x04); // offset +4
        let cycles = cpu.step(&mut mmu);
        assert_eq!(cycles, 8);
        assert_eq!(cpu.pc, 0x0102, "PC should advance by 2 only");
    }
}
