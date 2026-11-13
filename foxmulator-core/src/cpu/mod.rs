pub mod alu;
pub mod decoding;
pub mod flags;
pub mod registers;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Cpu {
    pub pc: u16,
    pub regs: crate::cpu::registers::Registers,
    pub ime: bool,
    /// Set by HALT/STOP until an enabled interrupt becomes pending.
    pub halted: bool,
    /// HALT bug: the byte after HALT is fetched again as its own operand.
    pub halt_bug: bool,
    /// Reserved SM83 opcodes lock the CPU until a hardware reset.
    pub locked: bool,
}

impl Default for Cpu {
    fn default() -> Self {
        Self::new()
    }
}

/// Address of the interrupt-request (IF) register.
const IF_ADDR: u16 = 0xFF0F;
/// Address of the interrupt-enable (IE) register.
const IE_ADDR: u16 = 0xFFFF;

/// Interrupt vectors, indexed by the bit number in IF/IE.
const INTERRUPT_VECTORS: [u16; 5] = [0x0040, 0x0048, 0x0050, 0x0058, 0x0060];

/// How an instruction accesses data memory (stack and PC fetches excluded).
#[derive(Clone, Copy, PartialEq, Eq)]
enum DataAccess {
    Read,
    Write,
    ReadModifyWrite,
}

/// Return the data-memory access kind and total T-cycles for an opcode.
///
/// Only the instructions that Blargg's `mem_timing` tests exercise are listed;
/// their total cycle counts are the documented SM83 timings. Instructions not
/// listed here have no data-memory access (or only stack/PC accesses).
fn data_access_plan(opcode: u8, mmu: &crate::mmu::Mmu, pc: u16) -> Option<(DataAccess, u32)> {
    use DataAccess::*;
    let plan = match opcode {
        // LD (BC/DE/HL±),A and LD A,(BC/DE/HL±)
        0x02 | 0x12 | 0x22 | 0x32 => (Write, 8),
        0x0A | 0x1A | 0x2A | 0x3A => (Read, 8),
        // INC/DEC (HL)
        0x34 | 0x35 => (ReadModifyWrite, 12),
        // LD (HL),d8
        0x36 => (Write, 12),
        // LD r,(HL)
        0x46 | 0x4E | 0x56 | 0x5E | 0x66 | 0x6E | 0x7E => (Read, 8),
        // LD (HL),r (0x76 is HALT and handled before the decoder)
        0x70..=0x75 | 0x77 => (Write, 8),
        // ALU A,(HL)
        0x86 | 0x8E | 0x96 | 0x9E | 0xA6 | 0xAE | 0xB6 | 0xBE => (Read, 8),
        // LDH (n),A / LDH (C),A
        0xE0 => (Write, 12),
        0xE2 => (Write, 8),
        // LD (nn),A
        0xEA => (Write, 16),
        // LDH A,(n) / LDH A,(C)
        0xF0 => (Read, 12),
        0xF2 => (Read, 8),
        // LD A,(nn)
        0xFA => (Read, 16),
        // CB-prefixed instructions.
        0xCB => {
            let sub = mmu.cpu_read_byte(pc.wrapping_add(1));
            let on_hl = (sub & 0x07) == 6;
            if !on_hl {
                return None;
            }
            match sub >> 6 {
                // BIT b,(HL): read on the last M-cycle.
                1 => (Read, 12),
                // RLC/RRC/RL/RR/SLA/SRA/SWAP/SRL (HL): read, modify, write.
                0 => (ReadModifyWrite, 16),
                // RES/SET b,(HL): read, modify, write.
                _ => (ReadModifyWrite, 16),
            }
        }
        _ => return None,
    };
    Some(plan)
}

impl Cpu {
    /// Create a new CPU instance.
    pub fn new() -> Self {
        Cpu {
            pc: 0,
            regs: crate::cpu::registers::Registers::default(),
            ime: true,
            halted: false,
            halt_bug: false,
            locked: false,
        }
    }

    /// Resets the CPU to the default boot state.
    pub fn reset(&mut self) {
        self.pc = 0x0100;
        self.regs.set_sp(0xFFFE);
        self.halted = false;
        self.halt_bug = false;
        self.locked = false;
        self.ime = false;
    }

    /// Enter the power-on state before executing a real boot ROM.
    pub fn start_boot_rom(&mut self) {
        self.pc = 0;
        self.regs = crate::cpu::registers::Registers::default();
        self.halted = false;
        self.halt_bug = false;
        self.locked = false;
        self.ime = false;
    }

    /// Apply the register state a real console has after the boot ROM hands
    /// control to the cartridge at 0x0100.
    ///
    /// DMG: A=0x01 F=0xB0 B=0x00 C=0x13 D=0x00 E=0xD8 H=0x01 L=0x4D.
    /// CGB: A=0x11 F=0x80 B=0x00 C=0x00 D=0xFF E=0x56 H=0x00 L=0x0D.
    /// Reference: Pan Docs, "Power Up Sequence".
    pub fn post_boot(&mut self, cgb: bool) {
        self.reset();
        if cgb {
            self.regs.a = 0x11;
            self.regs.f = 0x80;
            self.regs.b = 0x00;
            self.regs.c = 0x00;
            self.regs.d = 0xFF;
            self.regs.e = 0x56;
            self.regs.h = 0x00;
            self.regs.l = 0x0D;
        } else {
            self.regs.a = 0x01;
            self.regs.f = 0xB0;
            self.regs.b = 0x00;
            self.regs.c = 0x13;
            self.regs.d = 0x00;
            self.regs.e = 0xD8;
            self.regs.h = 0x01;
            self.regs.l = 0x4D;
        }
    }

    /// Pending enabled interrupts (IF & IE, low five bits).
    fn pending_interrupts(&self, mmu: &crate::mmu::Mmu) -> u8 {
        mmu.cpu_read_byte(IF_ADDR) & mmu.cpu_read_byte(IE_ADDR) & 0x1F
    }

    /// Service the highest-priority pending interrupt.
    /// Returns the consumed T-cycles, or `None` if no interrupt was serviced.
    fn service_interrupt(&mut self, mmu: &mut crate::mmu::Mmu) -> Option<u32> {
        if !self.ime {
            return None;
        }
        let pending = self.pending_interrupts(mmu);
        if pending == 0 {
            return None;
        }

        let bit = pending.trailing_zeros() as usize;
        // Clear the serviced request bit.
        let if_reg = mmu.cpu_read_byte(IF_ADDR);
        mmu.cpu_write_byte(IF_ADDR, if_reg & !(1 << bit));

        // Push the return address and jump to the vector.
        let sp = self.regs.sp().wrapping_sub(2);
        self.regs.set_sp(sp);
        mmu.cpu_write_byte(sp, (self.pc & 0xFF) as u8);
        mmu.cpu_write_byte(sp.wrapping_add(1), (self.pc >> 8) as u8);
        self.pc = INTERRUPT_VECTORS[bit];
        self.ime = false;
        self.halted = false;
        self.halt_bug = false;

        mmu.tick(20);
        Some(20)
    }

    /// Execute a single instruction.
    /// Fetches the opcode from the MMU, dispatches to the decoder and
    /// returns the number of clock cycles the instruction consumes.
    pub fn step(&mut self, mmu: &mut crate::mmu::Mmu) -> u32 {
        // Unlike HALT, an illegal SM83 opcode cannot be woken by an
        // interrupt. Keep advancing peripherals at the instruction clock so
        // headless runners and audio/video state remain deterministic, but do
        // not fetch, service interrupts, or mutate CPU state again.
        if self.locked {
            mmu.tick(4);
            return 4;
        }

        if let Some(cycles) = self.service_interrupt(mmu) {
            return cycles;
        }

        // A halted CPU stays idle until an enabled interrupt is requested.
        if self.halted {
            if self.pending_interrupts(mmu) != 0 {
                // Wake up and continue execution; the interrupt itself is
                // serviced on the next step if IME is set.
                self.halted = false;
            } else {
                mmu.tick(4);
                return 4;
            }
        }

        let instruction_pc = self.pc;
        let opcode = mmu.cpu_read_byte(self.pc);

        if matches!(
            opcode,
            0xD3 | 0xDB | 0xDD | 0xE3 | 0xE4 | 0xEB | 0xEC | 0xED | 0xF4 | 0xFC | 0xFD
        ) {
            // The opcode fetch itself has completed. The CPU then enters the
            // permanent lock-up state documented by Pan Docs; no operand or
            // following instruction is fetched.
            self.pc = self.pc.wrapping_add(1);
            self.locked = true;
            mmu.tick(4);
            return 4;
        }

        // HALT (0x76) and STOP (0x10) are core-level states.
        if opcode == 0x76 {
            self.pc = self.pc.wrapping_add(1);
            // HALT bug: if IME is clear and an interrupt is already pending,
            // the CPU does not halt and the byte after HALT is fetched twice.
            // Pan Docs, "HALT".
            if !self.ime && self.pending_interrupts(mmu) != 0 {
                self.halt_bug = true;
            } else {
                self.halted = true;
            }
            mmu.tick(4);
            return 4;
        }
        if opcode == 0x10 {
            // STOP is a two-byte instruction (0x10 0x00).
            self.pc = self.pc.wrapping_add(2);
            // On CGB, STOP performs the KEY1 speed switch when it has been
            // armed instead of halting the CPU. Pan Docs, "KEY1".
            if mmu.cgb && mmu.key1_armed {
                mmu.double_speed = !mmu.double_speed;
                mmu.key1_armed = false;
                mmu.tick(4);
                return 4;
            }
            self.halted = true;
            mmu.tick(4);
            return 4;
        }

        // Place data-memory accesses on their real M-cycles. PC and stack
        // accesses are excluded from Blargg's timing tests, so the timer only
        // has to be advanced to the access cycle before the decoder runs.
        let plan = data_access_plan(opcode, mmu, self.pc);
        let stack_pointer_before = self.regs.sp();
        let oam_address = match opcode {
            // 16-bit IDU operations use the value before the operation.
            0x03 | 0x0B => Some(self.regs.bc()),
            0x13 | 0x1B => Some(self.regs.de()),
            0x23 | 0x2B => Some(self.regs.hl()),
            0x33 | 0x3B => Some(self.regs.sp()),
            // Stack operations expose SP on the bus during their implicit
            // increment/decrement. POP from $FDFF therefore reaches $FE00
            // even though its first stack read uses the pre-operation SP.
            0xC1 | 0xD1 | 0xE1 | 0xF1 => {
                let sp = self.regs.sp();
                Some(
                    if !(0xFE00..=0xFEFF).contains(&sp)
                        && (0xFE00..=0xFEFF).contains(&sp.wrapping_add(1))
                    {
                        sp.wrapping_add(1)
                    } else {
                        sp
                    },
                )
            }
            0xC5 | 0xD5 | 0xE5 | 0xF5 => Some(self.regs.sp()),
            // HL auto-index operations use the address before HL changes.
            0x22 | 0x32 | 0x2A | 0x3A => Some(self.regs.hl()),
            _ => None,
        };
        let oam_read = matches!(opcode, 0x2A | 0x3A | 0xC1 | 0xD1 | 0xE1 | 0xF1);
        let oam_pop = matches!(opcode, 0xC1 | 0xD1 | 0xE1 | 0xF1);
        let oam_write = matches!(
            opcode,
            0x03 | 0x0B
                | 0x13
                | 0x1B
                | 0x23
                | 0x2B
                | 0x33
                | 0x3B
                | 0x22
                | 0x32
                | 0xC5
                | 0xD5
                | 0xE5
                | 0xF5
        );
        if let Some((access, total)) = plan {
            match access {
                DataAccess::Read | DataAccess::Write => mmu.tick(total - 4),
                DataAccess::ReadModifyWrite => {
                    // Read on the next-to-last M-cycle, write on the last.
                    mmu.tick(total - 8);
                    mmu.pre_write_tick = 4;
                }
            }
        }

        // POP's first stack read is a real bus event before the decoder reads
        // the second byte. The decoder API intentionally does not expose
        // individual stack M-cycles, so place the first OAM trigger here and
        // the second one immediately after the decoder below.
        let oam_pop_active =
            oam_pop && oam_address.is_some_and(|address| (0xFE00..=0xFEFF).contains(&address));
        if oam_pop_active {
            mmu.tick(4);
            if std::env::var_os("FOX_OAM_TRACE").is_some() {
                let (ticks, line, mode, row) = mmu.oam_debug_state();
                eprintln!(
                    "OAM_TRACE pc={instruction_pc:04X} op={opcode:02X} kind=pop-first microop=read addr={stack_pointer_before:04X} sp_before={stack_pointer_before:04X} ticks={ticks} phase={} ly={:02X} line_cycles={line} mode={mode} scan_row={row:02X} branch=read-normal simultaneous=false",
                    ticks % 4,
                    mmu.cpu_read_byte(crate::mmu::LY_ADDR)
                );
            }
            mmu.oam_bug_read_context(Some((
                stack_pointer_before,
                stack_pointer_before,
                "pop-first",
            )));
        }
        // The decoder will update pc and registers.
        if self.halt_bug {
            // Skip the opcode-consumption increment so the first operand fetch
            // reads the opcode byte again.
            self.halt_bug = false;
            self.pc = self.pc.wrapping_sub(1);
        }
        let cycles =
            crate::cpu::decoding::exec(opcode, &mut self.regs, &mut self.pc, mmu, &mut self.ime);

        let mut oam_clocked = false;
        if let Some(address) = oam_address {
            if (0xFE00..=0xFEFF).contains(&address) {
                // INC/DEC rr and stack IDU accesses occupy the final internal
                // M-cycle. Data accesses have already been positioned by the
                // plan above; the remaining operations need one explicit
                // M-cycle before applying the hardware corruption.
                if oam_pop_active && plan.is_none() {
                    // POP's second stack read occupies the next M-cycle.
                    mmu.tick(4);
                    if std::env::var_os("FOX_OAM_TRACE").is_some() {
                        let (ticks, line, mode, row) = mmu.oam_debug_state();
                        eprintln!(
                            "OAM_TRACE pc={instruction_pc:04X} op={opcode:02X} kind=pop-second microop=read addr={:04X} sp_before={:04X} ticks={ticks} phase={} ly={:02X} line_cycles={line} mode={mode} scan_row={row:02X} branch=read-normal simultaneous=false",
                            stack_pointer_before.wrapping_add(1),
                            stack_pointer_before.wrapping_add(1),
                            ticks % 4,
                            mmu.cpu_read_byte(crate::mmu::LY_ADDR)
                        );
                    }
                    mmu.oam_bug_read_context(Some((
                        stack_pointer_before.wrapping_add(1),
                        stack_pointer_before.wrapping_add(1),
                        "pop-second",
                    )));
                    mmu.tick(4);
                } else {
                    if plan.is_none() {
                        mmu.tick(cycles);
                    }
                    if oam_read {
                        if std::env::var_os("FOX_OAM_TRACE").is_some()
                            && matches!(opcode, 0xC1 | 0xD1 | 0xE1 | 0xF1)
                        {
                            let (ticks, line, mode, row) = mmu.oam_debug_state();
                            eprintln!(
                            "OAM_TRACE pc={instruction_pc:04X} op={opcode:02X} kind=read addr={address:04X} sp_before={stack_pointer_before:04X} sp_after={:04X} ticks={ticks} line={line} mode={mode} row={row:02X}",
                            self.regs.sp()
                        );
                        }
                        mmu.oam_bug_read();
                    } else if oam_write {
                        if std::env::var_os("FOX_OAM_TRACE").is_some() {
                            let (ticks, line, mode, row) = mmu.oam_debug_state();
                            eprintln!(
                            "OAM_TRACE op={opcode:02X} kind=write sp={:04X} ticks={ticks} line={line} mode={mode} row={row:02X}",
                            self.regs.sp()
                        );
                        }
                        mmu.oam_bug_write();
                    }
                }
                if plan.is_none() {
                    oam_clocked = true;
                }
            }
        }

        // Finish the instruction. For data accesses the access itself happened
        // one M-cycle before the end; otherwise advance the whole instruction.
        // Clear any unused mid-write tick so it cannot leak into a later write.
        mmu.pre_write_tick = 0;
        if plan.is_some() {
            mmu.tick(4);
        } else if !oam_clocked {
            mmu.tick(cycles);
        }
        cycles
    }
}
