#[cfg(test)]
mod tests {
    use foxmulator_core::{cpu::Cpu, mmu::Mmu};

    #[test]
    fn cpu_call_ret() {
        let mut mmu = Mmu::new();
        let mut cpu = Cpu::new();
        cpu.reset();
        // Write CALL 0x0600 at 0x0100
        mmu.write_byte(0x0100, 0xCD); // CALL
        mmu.write_byte(0x0101, 0x00); // low byte of address
        mmu.write_byte(0x0102, 0x06); // high byte of address

        // At 0x0600: NOP
        mmu.write_byte(0x0600, 0x00);
        // At 0x0601: RET
        mmu.write_byte(0x0601, 0xC9);

        // Step CALL
        let cycles = cpu.step(&mut mmu);
        assert_eq!(cycles, 24, "CALL should take 24 cycles");
        assert_eq!(cpu.pc, 0x0600, "PC should jump to target");
        assert_eq!(
            cpu.regs.sp, 0xFFFC,
            "SP should be decreased by 2 after push"
        );

        // Step NOP
        let cycles = cpu.step(&mut mmu);
        assert_eq!(cycles, 4, "NOP should take 4 cycles");
        assert_eq!(cpu.pc, 0x0601);

        // Step RET
        let cycles = cpu.step(&mut mmu);
        // Pan Docs / gbdev.io/gb-opcodes: RET (0xC9) takes 16 T-cycles.
        assert_eq!(cycles, 16, "RET should take 16 cycles");
        assert_eq!(cpu.pc, 0x0103, "PC should return to address after CALL");
        assert_eq!(cpu.regs.sp, 0xFFFE, "SP should be restored");
    }

    #[test]
    fn cpu_rst() {
        let mut mmu = Mmu::new();
        let mut cpu = Cpu::new();
        cpu.reset();
        // Write RST 0x00 at 0x0100
        mmu.write_byte(0x0100, 0xC7);
        let cycles = cpu.step(&mut mmu);
        assert_eq!(cycles, 16, "RST should take 16 cycles");
        assert_eq!(cpu.pc, 0x0000, "PC should jump to address 0x00");
        // Verify stack contains return address 0x0101
        let low = mmu.read_byte(0xFFFC);
        let high = mmu.read_byte(0xFFFD);
        let ret_addr = ((high as u16) << 8) | (low as u16);
        assert_eq!(ret_addr, 0x0101, "Stack should hold return address");
        assert_eq!(cpu.regs.sp, 0xFFFC, "SP should point to stack after push");
    }
}
