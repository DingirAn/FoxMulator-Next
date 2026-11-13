#[cfg(test)]
mod extra_opcodes {
    use foxmulator_core::{cpu::Cpu, mmu::Mmu};

    #[test]
    fn cpu_ld_a_c() {
        let mut mmu = Mmu::new();
        let mut cpu = Cpu::new();
        cpu.reset();
        mmu.write_byte(0x0100, 0x79); // LD A,C
        cpu.regs.c = 0x11;
        let cycles = cpu.step(&mut mmu);
        assert_eq!(cycles, 4);
        assert_eq!(cpu.regs.a, 0x11);
        assert_eq!(cpu.pc, 0x0101);
    }

    #[test]
    fn cpu_ld_a_d() {
        let mut mmu = Mmu::new();
        let mut cpu = Cpu::new();
        cpu.reset();
        mmu.write_byte(0x0100, 0x7A); // LD A,D
        cpu.regs.d = 0x22;
        let cycles = cpu.step(&mut mmu);
        assert_eq!(cycles, 4);
        assert_eq!(cpu.regs.a, 0x22);
        assert_eq!(cpu.pc, 0x0101);
    }

    #[test]
    fn cpu_ld_a_e() {
        let mut mmu = Mmu::new();
        let mut cpu = Cpu::new();
        cpu.reset();
        mmu.write_byte(0x0100, 0x7B); // LD A,E
        cpu.regs.e = 0x33;
        let cycles = cpu.step(&mut mmu);
        assert_eq!(cycles, 4);
        assert_eq!(cpu.regs.a, 0x33);
        assert_eq!(cpu.pc, 0x0101);
    }

    #[test]
    fn cpu_ld_a_h() {
        let mut mmu = Mmu::new();
        let mut cpu = Cpu::new();
        cpu.reset();
        mmu.write_byte(0x0100, 0x7C); // LD A,H
        cpu.regs.h = 0x44;
        let cycles = cpu.step(&mut mmu);
        assert_eq!(cycles, 4);
        assert_eq!(cpu.regs.a, 0x44);
        assert_eq!(cpu.pc, 0x0101);
    }

    #[test]
    fn cpu_ld_a_l() {
        let mut mmu = Mmu::new();
        let mut cpu = Cpu::new();
        cpu.reset();
        mmu.write_byte(0x0100, 0x7D); // LD A,L
        cpu.regs.l = 0x55;
        let cycles = cpu.step(&mut mmu);
        assert_eq!(cycles, 4);
        assert_eq!(cpu.regs.a, 0x55);
        assert_eq!(cpu.pc, 0x0101);
    }

    #[test]
    fn cpu_and_a_b() {
        let mut mmu = Mmu::new();
        let mut cpu = Cpu::new();
        cpu.reset();
        cpu.regs.a = 0b10101010;
        cpu.regs.b = 0b11001100;
        mmu.write_byte(0x0100, 0xA0); // AND A,B
        let cycles = cpu.step(&mut mmu);
        assert_eq!(cycles, 4);
        assert_eq!(cpu.regs.a, 0b10001000);
        assert_eq!(cpu.regs.f & 0x80, 0x00); // Zero flag cleared
    }

    #[test]
    fn cpu_or_a_b() {
        let mut mmu = Mmu::new();
        let mut cpu = Cpu::new();
        cpu.reset();
        cpu.regs.a = 0b10101010;
        cpu.regs.b = 0b01000101;
        mmu.write_byte(0x0100, 0xB0); // OR A,B
        let cycles = cpu.step(&mut mmu);
        assert_eq!(cycles, 4);
        assert_eq!(cpu.regs.a, 0b11101111);
    }

    #[test]
    fn cpu_cp_a_b() {
        let mut mmu = Mmu::new();
        let mut cpu = Cpu::new();
        cpu.reset();
        cpu.regs.a = 0x10;
        cpu.regs.b = 0x10;
        mmu.write_byte(0x0100, 0xB8); // CP A,B
        let cycles = cpu.step(&mut mmu);
        assert_eq!(cycles, 4);
        assert_eq!(cpu.regs.f & 0x80, 0x80); // Zero flag set
    }

    #[test]
    fn cpu_jp_nn() {
        let mut mmu = Mmu::new();
        let mut cpu = Cpu::new();
        cpu.reset();
        let target: u16 = 0x1234;
        mmu.write_byte(0x0100, 0xC3); // JP nn
        mmu.write_byte(0x0101, (target & 0xFF) as u8);
        mmu.write_byte(0x0102, (target >> 8) as u8);
        let cycles = cpu.step(&mut mmu);
        // JP a16 (0xC3) takes 16 T-cycles (Pan Docs).
        assert_eq!(cycles, 16);
        assert_eq!(cpu.pc, target);
    }

    #[test]
    fn cpu_ld_a_hl() {
        let mut mmu = Mmu::new();
        let mut cpu = Cpu::new();
        cpu.reset();
        // Set address pointed by HL
        cpu.regs.h = 0x02;
        cpu.regs.l = 0x00;
        mmu.write_byte(0x0200, 0x42);
        mmu.write_byte(0x0100, 0x7E); // LD A,(HL)
        let cycles = cpu.step(&mut mmu);
        assert_eq!(cycles, 8);
        assert_eq!(cpu.regs.a, 0x42);
        assert_eq!(cpu.pc, 0x0101);
    }

    #[test]
    fn cpu_add_a_hl() {
        let mut mmu = Mmu::new();
        let mut cpu = Cpu::new();
        cpu.reset();
        cpu.regs.a = 0x34;
        cpu.regs.h = 0x02;
        cpu.regs.l = 0x00;
        mmu.write_byte(0x0200, 0x12);
        mmu.write_byte(0x0100, 0x86); // ADD A,(HL)
        let cycles = cpu.step(&mut mmu);
        assert_eq!(cycles, 8);
        assert_eq!(cpu.regs.a, 0x46); // 0x34 + 0x12
    }

    #[test]
    fn cpu_dec_b() {
        let mut mmu = Mmu::new();
        let mut cpu = Cpu::new();
        cpu.reset();
        cpu.regs.b = 0x01;
        mmu.write_byte(0x0100, 0x05); // DEC B
        let cycles = cpu.step(&mut mmu);
        assert_eq!(cycles, 4);
        assert_eq!(cpu.regs.b, 0x00);
        assert_eq!(cpu.regs.f & 0x80, 0x80); // Zero
        assert_eq!(cpu.regs.f & 0x40, 0x40); // N
    }

    #[test]
    fn cpu_dec_c() {
        let mut mmu = Mmu::new();
        let mut cpu = Cpu::new();
        cpu.reset();
        cpu.regs.c = 0x01;
        mmu.write_byte(0x0100, 0x0D); // DEC C
        let cycles = cpu.step(&mut mmu);
        assert_eq!(cycles, 4);
        assert_eq!(cpu.regs.c, 0x00);
        assert_eq!(cpu.regs.f & 0x80, 0x80);
        assert_eq!(cpu.regs.f & 0x40, 0x40);
    }

    #[test]
    fn cpu_dec_d() {
        let mut mmu = Mmu::new();
        let mut cpu = Cpu::new();
        cpu.reset();
        cpu.regs.d = 0x01;
        mmu.write_byte(0x0100, 0x15); // DEC D
        let cycles = cpu.step(&mut mmu);
        assert_eq!(cycles, 4);
        assert_eq!(cpu.regs.d, 0x00);
        assert_eq!(cpu.regs.f & 0x80, 0x80);
        assert_eq!(cpu.regs.f & 0x40, 0x40);
    }

    #[test]
    fn cpu_dec_e() {
        let mut mmu = Mmu::new();
        let mut cpu = Cpu::new();
        cpu.reset();
        cpu.regs.e = 0x01;
        mmu.write_byte(0x0100, 0x1D); // DEC E
        let cycles = cpu.step(&mut mmu);
        assert_eq!(cycles, 4);
        assert_eq!(cpu.regs.e, 0x00);
        assert_eq!(cpu.regs.f & 0x80, 0x80);
        assert_eq!(cpu.regs.f & 0x40, 0x40);
    }

    #[test]
    fn cpu_dec_h() {
        let mut mmu = Mmu::new();
        let mut cpu = Cpu::new();
        cpu.reset();
        cpu.regs.h = 0x01;
        mmu.write_byte(0x0100, 0x25); // DEC H
        let cycles = cpu.step(&mut mmu);
        assert_eq!(cycles, 4);
        assert_eq!(cpu.regs.h, 0x00);
        assert_eq!(cpu.regs.f & 0x80, 0x80);
        assert_eq!(cpu.regs.f & 0x40, 0x40);
    }

    #[test]
    fn cpu_dec_l() {
        let mut mmu = Mmu::new();
        let mut cpu = Cpu::new();
        cpu.reset();
        cpu.regs.l = 0x01;
        mmu.write_byte(0x0100, 0x2D); // DEC L
        let cycles = cpu.step(&mut mmu);
        assert_eq!(cycles, 4);
        assert_eq!(cpu.regs.l, 0x00);
        assert_eq!(cpu.regs.f & 0x80, 0x80);
        assert_eq!(cpu.regs.f & 0x40, 0x40);
    }

    #[test]
    fn cpu_dec_a() {
        let mut mmu = Mmu::new();
        let mut cpu = Cpu::new();
        cpu.reset();
        cpu.regs.a = 0x01;
        mmu.write_byte(0x0100, 0x3D); // DEC A
        let cycles = cpu.step(&mut mmu);
        assert_eq!(cycles, 4);
        assert_eq!(cpu.regs.a, 0x00);
        assert_eq!(cpu.regs.f & 0x80, 0x80);
        assert_eq!(cpu.regs.f & 0x40, 0x40);
    }
}
