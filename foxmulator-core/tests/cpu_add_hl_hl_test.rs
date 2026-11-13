#[cfg(test)]
mod tests {
    use foxmulator_core::cpu::flags::Flags;
    use foxmulator_core::{cpu::Cpu, mmu::Mmu};

    #[test]
    fn cpu_add_hl_hl_no_carry() {
        let mut mmu = Mmu::new();
        let mut cpu = Cpu::new();
        cpu.reset();
        cpu.regs.set_hl(0x1234);
        cpu.regs.f = 0xFF; // arbitrary
        mmu.write_byte(0x0100, 0x29); // ADD HL,HL
        let cycles = cpu.step(&mut mmu);
        assert_eq!(cycles, 8, "ADD HL,HL should take 8 cycles");
        assert_eq!(cpu.regs.hl(), 0x2468, "Result HL should be 0x2468");
        assert_eq!(cpu.pc, 0x0101);
        // Z preserved, N cleared, H/C recomputed (no H, no C here).
        assert_eq!(cpu.regs.f, Flags::Z, "Only Z is preserved");
    }

    #[test]
    fn cpu_add_hl_hl_with_carry() {
        let mut mmu = Mmu::new();
        let mut cpu = Cpu::new();
        cpu.reset();
        cpu.regs.set_hl(0xA200);
        cpu.regs.f = 0xAB; // arbitrary
        mmu.write_byte(0x0100, 0x29);
        let cycles = cpu.step(&mut mmu);
        assert_eq!(cycles, 8, "ADD HL,HL should take 8 cycles");
        assert_eq!(cpu.regs.hl(), 0x4400, "Result HL should be 0x4400");
        assert_eq!(cpu.pc, 0x0101);
        // Z preserved from 0xAB, C set by 0xA200+0xA200, N cleared, H clear.
        assert_eq!(cpu.regs.f, Flags::Z | Flags::C, "Z preserved, C set");
    }
}
