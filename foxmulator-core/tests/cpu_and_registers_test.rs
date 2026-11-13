#[cfg(test)]
mod tests {
    use foxmulator_core::cpu::flags::Flags;
    use foxmulator_core::{cpu::Cpu, mmu::Mmu};

    #[test]
    fn cpu_and_b() {
        let mut mmu = Mmu::new();
        let mut cpu = Cpu::new();
        cpu.reset();
        cpu.regs.a = 0xF0;
        cpu.regs.b = 0x0F;
        cpu.regs.f = 0xAB; // random
        let original_sp = cpu.regs.sp();
        mmu.write_byte(0x0100, 0xA0); // AND B
        let cycles = cpu.step(&mut mmu);
        assert_eq!(cycles, 4, "AND B should take 4 cycles");
        assert_eq!(cpu.pc, 0x0101, "PC should advance");
        assert_eq!(cpu.regs.sp(), original_sp, "SP unchanged");
        assert_eq!(cpu.regs.a, 0x00, "Result zero");
        assert_eq!(cpu.regs.f & 0x80, 0x80, "Zero flag set");
        assert_eq!(cpu.regs.f & Flags::H, Flags::H, "Half carry set");
        assert_eq!(cpu.regs.f & Flags::N, 0, "N flag cleared");
        assert_eq!(cpu.regs.f & Flags::C, 0, "Carry cleared");
    }

    #[test]
    fn cpu_and_c() {
        let mut mmu = Mmu::new();
        let mut cpu = Cpu::new();
        cpu.reset();
        cpu.regs.a = 0xFF;
        cpu.regs.c = 0x0F;
        let original_sp = cpu.regs.sp();
        mmu.write_byte(0x0100, 0xA1); // AND C
        let cycles = cpu.step(&mut mmu);
        assert_eq!(cycles, 4, "AND C should take 4 cycles");
        assert_eq!(cpu.pc, 0x0101, "PC should advance");
        assert_eq!(cpu.regs.sp(), original_sp, "SP unchanged");
        assert_eq!(cpu.regs.a, 0x0F, "Result");
        assert_eq!(cpu.regs.f & 0x80, 0x00, "Zero flag cleared");
        assert_eq!(cpu.regs.f & Flags::H, Flags::H, "Half carry set");
        assert_eq!(cpu.regs.f & Flags::C, 0, "Carry cleared");
    }

    #[test]
    fn cpu_and_d() {
        let mut mmu = Mmu::new();
        let mut cpu = Cpu::new();
        cpu.reset();
        cpu.regs.a = 0x7F;
        cpu.regs.d = 0xF0;
        let original_sp = cpu.regs.sp();
        mmu.write_byte(0x0100, 0xA2); // AND D
        let cycles = cpu.step(&mut mmu);
        assert_eq!(cycles, 4, "AND D should take 4 cycles");
        assert_eq!(cpu.pc, 0x0101, "PC should advance");
        assert_eq!(cpu.regs.sp(), original_sp, "SP unchanged");
        assert_eq!(cpu.regs.a, 0x70, "Result");
        assert_eq!(cpu.regs.f & 0x80, 0x00, "Zero flag cleared");
        assert_eq!(cpu.regs.f & Flags::H, Flags::H, "Half carry set");
        assert_eq!(cpu.regs.f & Flags::C, 0, "Carry cleared");
    }

    #[test]
    fn cpu_and_e() {
        let mut mmu = Mmu::new();
        let mut cpu = Cpu::new();
        cpu.reset();
        cpu.regs.a = 0x80;
        cpu.regs.e = 0x80;
        let original_sp = cpu.regs.sp();
        mmu.write_byte(0x0100, 0xA3); // AND E
        let cycles = cpu.step(&mut mmu);
        assert_eq!(cycles, 4, "AND E should take 4 cycles");
        assert_eq!(cpu.pc, 0x0101, "PC should advance");
        assert_eq!(cpu.regs.sp(), original_sp, "SP unchanged");
        assert_eq!(cpu.regs.a, 0x80, "Result");
        assert_eq!(cpu.regs.f & 0x80, 0x00, "Zero flag cleared");
        assert_eq!(cpu.regs.f & Flags::H, Flags::H, "Half carry set");
        assert_eq!(cpu.regs.f & Flags::C, 0, "Carry cleared");
    }
}
