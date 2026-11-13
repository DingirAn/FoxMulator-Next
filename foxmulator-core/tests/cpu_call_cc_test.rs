#[cfg(test)]
mod tests {
    use foxmulator_core::cpu::flags::Flags;
    use foxmulator_core::{cpu::Cpu, mmu::Mmu};

    // Helper to get return address from stack
    fn get_return_addr(mmu: &Mmu, sp: u16) -> u16 {
        let low = mmu.read_byte(sp);
        let high = mmu.read_byte(sp.wrapping_add(1));
        ((high as u16) << 8) | (low as u16)
    }

    #[test]
    fn cpu_call_nz_taken() {
        let mut mmu = Mmu::new();
        let mut cpu = Cpu::new();
        cpu.reset();
        // Write CALL NZ,0x0200 at 0x0100 (0xC4 = CALL NZ; 0xC2 is JP NZ)
        mmu.write_byte(0x0100, 0xC4); // CALL NZ
        mmu.write_byte(0x0101, 0x00); // low
        mmu.write_byte(0x0102, 0x02); // high - 0x0200
                                      // Ensure Z flag clear
        cpu.regs.f = 0;

        let cycles = cpu.step(&mut mmu);
        assert_eq!(cycles, 24, "CALL NZ should take 24 cycles when taken");
        assert_eq!(cpu.pc, 0x0200);
        assert_eq!(
            cpu.regs.sp, 0xFFFC,
            "SP should be decreased by 2 after push"
        );
        let ret_addr = get_return_addr(&mmu, cpu.regs.sp());
        assert_eq!(ret_addr, 0x0103, "Return address should be 0x0103");
    }

    #[test]
    fn cpu_call_nz_not_taken() {
        let mut mmu = Mmu::new();
        let mut cpu = Cpu::new();
        cpu.reset();
        // Write CALL NZ,0x0200
        mmu.write_byte(0x0100, 0xC2);
        mmu.write_byte(0x0101, 0x00);
        mmu.write_byte(0x0102, 0x02);
        // Set Z flag to 1 -> condition not met
        cpu.regs.f = 0x80;

        let cycles = cpu.step(&mut mmu);
        assert_eq!(cycles, 12, "CALL NZ should take 12 cycles when not taken");
        assert_eq!(cpu.pc, 0x0103, "PC should skip over call instruction");
        assert_eq!(cpu.regs.sp, 0xFFFE, "SP should remain unchanged");
    }

    #[test]
    fn cpu_call_z_taken() {
        let mut mmu = Mmu::new();
        let mut cpu = Cpu::new();
        cpu.reset();
        mmu.write_byte(0x0100, 0xCC); // CALL Z
        mmu.write_byte(0x0101, 0x00);
        mmu.write_byte(0x0102, 0x02);
        cpu.regs.f = 0x80; // Z set
        let cycles = cpu.step(&mut mmu);
        assert_eq!(cycles, 24);
        assert_eq!(cpu.pc, 0x0200);
        assert_eq!(cpu.regs.sp, 0xFFFC);
        assert_eq!(get_return_addr(&mmu, cpu.regs.sp()), 0x0103);
    }

    #[test]
    fn cpu_call_nc_taken() {
        let mut mmu = Mmu::new();
        let mut cpu = Cpu::new();
        cpu.reset();
        mmu.write_byte(0x0100, 0xD4); // CALL NC
        mmu.write_byte(0x0101, 0x00);
        mmu.write_byte(0x0102, 0x02);
        cpu.regs.f = 0x00; // C clear
        let cycles = cpu.step(&mut mmu);
        assert_eq!(cycles, 24);
        assert_eq!(cpu.pc, 0x0200);
        assert_eq!(cpu.regs.sp, 0xFFFC);
        assert_eq!(get_return_addr(&mmu, cpu.regs.sp()), 0x0103);
    }

    #[test]
    fn cpu_call_c_taken() {
        let mut mmu = Mmu::new();
        let mut cpu = Cpu::new();
        cpu.reset();
        mmu.write_byte(0x0100, 0xDC); // CALL C
        mmu.write_byte(0x0101, 0x00);
        mmu.write_byte(0x0102, 0x02);
        cpu.regs.f = Flags::C; // C set (bit 4, not bit 0)
        let cycles = cpu.step(&mut mmu);
        assert_eq!(cycles, 24);
        assert_eq!(cpu.pc, 0x0200);
        assert_eq!(cpu.regs.sp, 0xFFFC);
        assert_eq!(get_return_addr(&mmu, cpu.regs.sp()), 0x0103);
    }
}
