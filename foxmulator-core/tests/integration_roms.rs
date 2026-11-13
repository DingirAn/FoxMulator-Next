#[cfg(test)]
mod integration_roms {
    use foxmulator_core::{cpu::Cpu, mmu::Mmu};

    // Simple ROM: A=0x10, B=0x20, A = A+B (0x30), halt
    const SIMPLE_ROM: [u8; 6] = [
        0x3E, 0x10, // LD A,0x10
        0x06, 0x20, // LD B,0x20
        0x80, // ADD A,B
        0x76, // HALT
    ];

    #[test]
    fn simple_rom_execution() {
        let mut mmu = Mmu::new();
        // Cpu::reset() starts execution at 0x0100 (DMG entry point after boot ROM).
        mmu.load_rom(0x0100, &SIMPLE_ROM);
        let mut cpu = Cpu::new();
        cpu.reset();
        // Execute until HALT is reached (steps 4 instructions)
        let mut cycles = 0;
        while cycles < 4 * 16 {
            // buffer
            let c = cpu.step(&mut mmu);
            cycles += c;
            if mmu.read_byte(cpu.pc) == 0x76 {
                // HALT
                break;
            }
        }
        assert_eq!(cpu.regs.a, 0x30);
        assert_eq!(cpu.regs.b, 0x20);
        assert_eq!(mmu.read_byte(cpu.pc), 0x76);
    }
}
