#[cfg(test)]
mod tests {
    use foxmulator_core::cpu::flags::Flags;
    use foxmulator_core::{cpu::Cpu, mmu::Mmu};

    #[test]
    fn cpu_add_hl_bc() {
        let mut mmu = Mmu::new();
        let mut cpu = Cpu::new();
        cpu.reset();
        // set registers
        cpu.regs.set_hl(0x1234);
        cpu.regs.set_bc(0x0100);
        cpu.regs.f = 0xFF; // arbitrary flags
                           // write opcode 0x09
        mmu.write_byte(0x0100, 0x09);
        let cycles = cpu.step(&mut mmu);
        assert_eq!(cycles, 8, "ADD HL,BC should take 8 cycles");
        assert_eq!(cpu.regs.hl(), 0x1334, "Result HL should be 0x1334");
        assert_eq!(cpu.pc, 0x0101, "PC should advance to 0x0101");
        // ADD HL,rr: Z is preserved, N is cleared, H/C recomputed.
        // 0x1234 + 0x0100 has no half-carry or carry, so only Z remains set.
        assert_eq!(cpu.regs.f, Flags::Z, "Only Z is preserved");
    }
}
