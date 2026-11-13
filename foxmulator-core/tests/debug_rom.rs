#[cfg(test)]
mod debug_rom {
    use foxmulator_core::{cpu::Cpu, mmu::Mmu};
    #[test]
    fn debug_simple_rom() {
        let mut mmu = Mmu::new();
        const SIMPLE_ROM: [u8; 6] = [0x3E, 0x10, 0x06, 0x20, 0x80, 0x76];
        mmu.load_rom(0x0000, &SIMPLE_ROM);
        let mut cpu = Cpu::new();
        cpu.reset();
        for i in 0..3 {
            let cycles = cpu.step(&mut mmu);
            println!(
                "step {}: pc={:04X} a={:02X} b={:02X} cycles={}",
                i, cpu.pc, cpu.regs.a, cpu.regs.b, cycles
            );
        }
        println!("final pc={:04X} a={:02X}", cpu.pc, cpu.regs.a);
    }
}
