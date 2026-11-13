#[cfg(test)]
mod tests {
    use foxmulator_core::cpu::flags::Flags;
    use foxmulator_core::{cpu::Cpu, mmu::Mmu};

    fn step_and_assert(cpu: &mut Cpu, mmu: &mut Mmu, cycles_expected: u32) {
        let cycles = cpu.step(mmu);
        assert_eq!(cycles, cycles_expected);
    }

    #[test]
    fn cpu_push_pop_bc() {
        let mut mmu = Mmu::new();
        let mut cpu = Cpu::new();
        cpu.reset();
        cpu.regs.set_bc(0x1234);
        mmu.write_byte(0x0100, 0xC5); // PUSH BC
        mmu.write_byte(0x0101, 0xC1); // POP BC
                                      // Execute PUSH
        step_and_assert(&mut cpu, &mut mmu, 16);
        // PUSH decrements SP by 2 (Pan Docs): 0xFFFE -> 0xFFFC.
        assert_eq!(cpu.regs.sp(), 0xFFFC);
        // Execute POP
        step_and_assert(&mut cpu, &mut mmu, 12);
        assert_eq!(cpu.regs.bc(), 0x1234);
        assert_eq!(cpu.pc, 0x0102);
    }

    #[test]
    fn cpu_call_nc_taken() {
        let mut mmu = Mmu::new();
        let mut cpu = Cpu::new();
        cpu.reset();
        // Ensure carry flag is clear
        cpu.regs.f &= !Flags::C;
        mmu.write_byte(0x0100, 0xC4); // CALL NC nn
        mmu.write_byte(0x0101, 0x34); // low
        mmu.write_byte(0x0102, 0x12); // high
        step_and_assert(&mut cpu, &mut mmu, 24);
        assert_eq!(cpu.pc, 0x1234);
        // Stack should contain return address 0x0103
        let sp = cpu.regs.sp();
        let ret_addr = ((mmu.read_byte(sp.wrapping_add(1)) as u16) << 8) | mmu.read_byte(sp) as u16;
        assert_eq!(ret_addr, 0x0103);
    }

    #[test]
    fn cpu_add_a_n() {
        let mut mmu = Mmu::new();
        let mut cpu = Cpu::new();
        cpu.reset();
        cpu.regs.a = 0x10;
        mmu.write_byte(0x0100, 0xC6); // ADD A,n
        mmu.write_byte(0x0101, 0x20);
        step_and_assert(&mut cpu, &mut mmu, 8);
        assert_eq!(cpu.regs.a, 0x30);
        assert_eq!(cpu.regs.f & Flags::Z, 0);
        assert_eq!(cpu.regs.f & Flags::H, 0);
        assert_eq!(cpu.regs.f & Flags::C, 0);
    }

    #[test]
    fn cpu_adc_a_n() {
        let mut mmu = Mmu::new();
        let mut cpu = Cpu::new();
        cpu.reset();
        cpu.regs.a = 0x10;
        cpu.regs.f = Flags::C; // carry set
        mmu.write_byte(0x0100, 0xCE); // ADC A,n
        mmu.write_byte(0x0101, 0x10);
        step_and_assert(&mut cpu, &mut mmu, 8);
        assert_eq!(cpu.regs.a, 0x21); // 0x10 + 0x10 + 1
        assert_eq!(cpu.regs.f & Flags::C, 0);
    }

    #[test]
    fn cpu_ret_z_taken() {
        let mut mmu = Mmu::new();
        let mut cpu = Cpu::new();
        cpu.reset();
        cpu.regs.f |= Flags::Z;
        // push return address 0x0500 onto stack
        cpu.regs.set_sp(0xFFFE);
        mmu.write_byte(0xFFFE, 0x00);
        mmu.write_byte(0xFFFF, 0x05);
        mmu.write_byte(0x0100, 0xC8); // RET Z
                                      // RET cc taken takes 20 T-cycles (16 + 4 for the taken branch).
        step_and_assert(&mut cpu, &mut mmu, 20);
        assert_eq!(cpu.pc, 0x0500);
    }

    #[test]
    fn cpu_pop_af_flags_mask() {
        let mut mmu = Mmu::new();
        let mut cpu = Cpu::new();
        cpu.reset();
        // Prepare value on stack
        cpu.regs.set_sp(0xFFFE);
        mmu.write_byte(0xFFFE, 0x12); // low byte of AF
        mmu.write_byte(0xFFFF, 0x34); // high byte of AF
        mmu.write_byte(0x0100, 0xF1); // POP AF
        step_and_assert(&mut cpu, &mut mmu, 12);
        assert_eq!(cpu.regs.a, 0x34);
        // lower nibble of F should be zero
        assert_eq!(cpu.regs.f & 0x0F, 0x00);
        // upper nibble should be 0x12
        assert_eq!(cpu.regs.f & 0xF0, 0x10);
    }
}
