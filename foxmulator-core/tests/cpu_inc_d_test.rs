#[cfg(test)]
mod tests {
    use foxmulator_core::{cpu::Cpu, mmu::Mmu};

    #[test]
    fn cpu_inc_d_zero_and_half_carry() {
        let mut mmu = Mmu::new();
        let mut cpu = Cpu::new();
        cpu.reset();
        // Setup: D=0xFF to test zero and half carry
        cpu.regs.d = 0xFF;
        cpu.regs.f = 0x00; // clear flags
        let original_sp = cpu.regs.sp();

        // Write opcode 0x14 (INC D)
        mmu.write_byte(0x0100, 0x14);
        let cycles = cpu.step(&mut mmu);

        assert_eq!(cycles, 4, "INC D should take 4 cycles");
        assert_eq!(cpu.pc, 0x0101, "PC should advance");
        assert_eq!(cpu.regs.sp(), original_sp, "SP unchanged");
        assert_eq!(cpu.regs.d, 0x00, "D should wrap to zero");
        assert_eq!(cpu.regs.f & 0x80, 0x80, "Zero flag set");
        assert_eq!(cpu.regs.f & 0x20, 0x20, "Half carry set");
        assert_eq!(cpu.regs.f & 0x40, 0x00, "N flag cleared");
    }

    #[test]
    fn cpu_inc_d_normal() {
        let mut mmu = Mmu::new();
        let mut cpu = Cpu::new();
        cpu.reset();
        cpu.regs.d = 0x10;
        cpu.regs.f = 0x00;
        let original_sp = cpu.regs.sp();

        mmu.write_byte(0x0100, 0x14);
        let cycles = cpu.step(&mut mmu);

        assert_eq!(cycles, 4, "INC D should take 4 cycles");
        assert_eq!(cpu.pc, 0x0101, "PC should advance");
        assert_eq!(cpu.regs.sp(), original_sp, "SP unchanged");
        assert_eq!(cpu.regs.d, 0x11, "D incremented");
        assert_eq!(cpu.regs.f & 0x80, 0x00, "Zero flag cleared");
        assert_eq!(cpu.regs.f & 0x20, 0x00, "Half carry cleared");
        assert_eq!(cpu.regs.f & 0x40, 0x00, "N flag cleared");
    }
}
