#[cfg(test)]
mod tests {
    use foxmulator_core::{cpu::Cpu, mmu::Mmu};

    #[test]
    fn cpu_dec_d_normal_no_half() {
        let mut mmu = Mmu::new();
        let mut cpu = Cpu::new();
        cpu.reset();
        cpu.regs.d = 0x11; // low nibble 0x1
        let original_sp = cpu.regs.sp();
        mmu.write_byte(0x0100, 0x15); // DEC D
        let cycles = cpu.step(&mut mmu);
        assert_eq!(cycles, 4, "DEC D should take 4 cycles");
        assert_eq!(cpu.pc, 0x0101, "PC should advance by 1");
        assert_eq!(cpu.regs.sp(), original_sp, "SP unchanged");
        assert_eq!(cpu.regs.d, 0x10, "D decremented by 1");
        assert_eq!(cpu.regs.f & 0x80, 0x00, "Zero flag cleared");
        assert_eq!(cpu.regs.f & 0x20, 0x00, "Half carry cleared");
        assert_eq!(cpu.regs.f & 0x40, 0x40, "N flag set");
    }

    #[test]
    fn cpu_dec_d_borrow_half() {
        let mut mmu = Mmu::new();
        let mut cpu = Cpu::new();
        cpu.reset();
        cpu.regs.d = 0x10; // low nibble 0x0, borrow occurs
        let _original_sp = cpu.regs.sp();
        mmu.write_byte(0x0100, 0x15);
        let _cycles = cpu.step(&mut mmu);
        assert_eq!(cpu.regs.d, 0x0F);
        assert_eq!(cpu.regs.f & 0x20, 0x20, "Half carry set");
        assert_eq!(cpu.regs.f & 0x40, 0x40, "N flag set");
    }

    #[test]
    fn cpu_dec_d_zero() {
        let mut mmu = Mmu::new();
        let mut cpu = Cpu::new();
        cpu.reset();
        cpu.regs.d = 0x01;
        let _original_sp = cpu.regs.sp();
        mmu.write_byte(0x0100, 0x15);
        let _cycles = cpu.step(&mut mmu);
        assert_eq!(cpu.regs.d, 0x00);
        assert_eq!(cpu.regs.f & 0x80, 0x80, "Zero flag set");
        assert_eq!(cpu.regs.f & 0x20, 0x00, "Half carry cleared");
        assert_eq!(cpu.regs.f & 0x40, 0x40, "N flag set");
    }

    #[test]
    fn cpu_dec_d_wrap_zero() {
        let mut mmu = Mmu::new();
        let mut cpu = Cpu::new();
        cpu.reset();
        cpu.regs.d = 0x00; // decrement wraps to 0xFF
        let _original_sp = cpu.regs.sp();
        mmu.write_byte(0x0100, 0x15);
        let _cycles = cpu.step(&mut mmu);
        assert_eq!(cpu.regs.d, 0xFF);
        assert_eq!(cpu.regs.f & 0x80, 0x00, "Zero flag cleared");
        assert_eq!(cpu.regs.f & 0x20, 0x20, "Half carry set");
        assert_eq!(cpu.regs.f & 0x40, 0x40, "N flag set");
    }
}
