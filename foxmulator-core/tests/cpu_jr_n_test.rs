#[cfg(test)]
mod tests {
    use foxmulator_core::{cpu::Cpu, mmu::Mmu};

    #[test]
    fn cpu_jr_n_positive_offset() {
        let mut mmu = Mmu::new();
        let mut cpu = Cpu::new();
        cpu.reset();
        cpu.regs.f = 0xAB; // preserve flags
        mmu.write_byte(0x0100, 0x18); // JR n
        mmu.write_byte(0x0101, 0x02); // +2
        let cycles = cpu.step(&mut mmu);
        assert_eq!(cycles, 12, "JR n should take 12 cycles");
        assert_eq!(cpu.pc, 0x0104, "PC should jump to 0x0104");
        assert_eq!(cpu.regs.f, 0xAB, "Flags should be unchanged");
    }

    #[test]
    fn cpu_jr_n_negative_offset() {
        let mut mmu = Mmu::new();
        let mut cpu = Cpu::new();
        cpu.reset();
        cpu.regs.f = 0xAB;
        mmu.write_byte(0x0100, 0x18);
        mmu.write_byte(0x0101, 0xFE); // -2
        let cycles = cpu.step(&mut mmu);
        assert_eq!(cycles, 12);
        assert_eq!(cpu.pc, 0x0100, "PC should return to start");
        assert_eq!(cpu.regs.f, 0xAB);
    }

    #[test]
    fn cpu_add_hl_de() {
        let mut mmu = Mmu::new();
        let mut cpu = Cpu::new();
        cpu.reset();
        cpu.regs.h = 0x02;
        cpu.regs.l = 0x00; // HL = 0x0200
        cpu.regs.d = 0x01;
        cpu.regs.e = 0x00; // DE = 0x0100
        let orig_f = cpu.regs.f;
        mmu.write_byte(0x0100, 0x19); // ADD HL,DE
        let cycles = cpu.step(&mut mmu);
        assert_eq!(cycles, 8, "ADD HL,DE should take 8 cycles");
        assert_eq!(cpu.regs.hl(), 0x0300, "HL should be 0x0300");
        assert_eq!(cpu.pc, 0x0101, "PC should advance by 1");
        assert_eq!(cpu.regs.f, orig_f, "Flags should be unchanged");
    }

    #[test]
    fn cpu_ld_a_of_hl() {
        let mut mmu = Mmu::new();
        let mut cpu = Cpu::new();
        cpu.reset();
        cpu.regs.h = 0x30;
        cpu.regs.l = 0x00; // HL = 0x3000
        mmu.write_byte(0x3000, 0xAB);
        let orig_f = cpu.regs.f;
        mmu.write_byte(0x0100, 0x7E); // LD A,(HL)
        let cycles = cpu.step(&mut mmu);
        assert_eq!(cycles, 8, "LD A,(HL) should take 8 cycles");
        assert_eq!(cpu.regs.a, 0xAB, "A should contain value from memory");
        assert_eq!(cpu.pc, 0x0101, "PC should advance by 1");
        assert_eq!(cpu.regs.f, orig_f, "Flags should be unchanged");
    }

    #[test]
    fn cpu_ld_hl_of_a() {
        let mut mmu = Mmu::new();
        let mut cpu = Cpu::new();
        cpu.reset();
        cpu.regs.h = 0x20;
        cpu.regs.l = 0x00; // HL = 0x2000
        cpu.regs.a = 0x55;
        let orig_f = cpu.regs.f;
        mmu.write_byte(0x0100, 0x77); // LD (HL),A
        let cycles = cpu.step(&mut mmu);
        assert_eq!(cycles, 8, "LD (HL),A should take 8 cycles");
        assert_eq!(
            mmu.read_byte(0x2000),
            0x55,
            "Memory at HL should contain 0x55"
        );
        assert_eq!(cpu.pc, 0x0101, "PC should advance by 1");
        assert_eq!(cpu.regs.f, orig_f, "Flags should be unchanged");
    }
}
