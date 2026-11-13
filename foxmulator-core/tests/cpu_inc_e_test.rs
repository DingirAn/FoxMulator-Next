#[cfg(test)]
mod tests {
    use foxmulator_core::{cpu::Cpu, mmu::Mmu};

    #[test]
    fn cpu_inc_e_normal_no_half() {
        let mut mmu = Mmu::new();
        let mut cpu = Cpu::new();
        cpu.reset();
        cpu.regs.e = 0x11;
        cpu.regs.f = 0x00;
        let original_sp = cpu.regs.sp();
        mmu.write_byte(0x0100, 0x1C); // INC E
        let cycles = cpu.step(&mut mmu);
        assert_eq!(cycles, 4, "INC E should take 4 cycles");
        assert_eq!(cpu.pc, 0x0101, "PC should advance by 1");
        assert_eq!(cpu.regs.sp(), original_sp, "SP unchanged");
        assert_eq!(cpu.regs.e, 0x12, "E incremented");
        assert_eq!(cpu.regs.f & 0x80, 0x00, "Zero flag cleared");
        assert_eq!(cpu.regs.f & 0x20, 0x00, "Half carry cleared");
        assert_eq!(cpu.regs.f & 0x40, 0x00, "N cleared");
    }

    #[test]
    fn cpu_inc_e_wrap_zero_and_half_carry() {
        let mut mmu = Mmu::new();
        let mut cpu = Cpu::new();
        cpu.reset();
        cpu.regs.e = 0xFF;
        cpu.regs.f = 0x00;
        let original_sp = cpu.regs.sp();
        mmu.write_byte(0x0100, 0x1C); // INC E
        let cycles = cpu.step(&mut mmu);
        assert_eq!(cycles, 4, "INC E should take 4 cycles");
        assert_eq!(cpu.pc, 0x0101, "PC should advance by 1");
        assert_eq!(cpu.regs.sp(), original_sp, "SP unchanged");
        assert_eq!(cpu.regs.e, 0x00, "E wraps to zero");
        assert_eq!(cpu.regs.f & 0x80, 0x80, "Zero flag set");
        assert_eq!(cpu.regs.f & 0x20, 0x20, "Half carry set");
        assert_eq!(cpu.regs.f & 0x40, 0x00, "N cleared");
    }
}
