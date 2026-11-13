//! Timer/DIV behavior derived from Pan Docs, "Timer and Divider Registers".

#[cfg(test)]
mod timer_tests {
    use foxmulator_core::mmu::{Mmu, DIV_ADDR, IF_ADDR, TAC_ADDR, TIMA_ADDR, TMA_ADDR};
    use foxmulator_core::timer_mod::Timer;

    #[test]
    fn div_increments_every_256_t_cycles() {
        let mut timer = Timer::new();
        assert_eq!(timer.div, 0);

        timer.tick(255);
        assert_eq!(timer.div, 0, "no increment before 256 cycles");

        timer.tick(1);
        assert_eq!(timer.div, 1, "increment exactly on the 256th cycle");

        timer.tick(256 * 3);
        assert_eq!(timer.div, 4);
    }

    #[test]
    fn div_write_resets_internal_counter() {
        let mut timer = Timer::new();
        timer.tick(300);
        assert_eq!(timer.div, 1);

        assert!(!timer.write_register(DIV_ADDR, 0x42));
        assert_eq!(timer.div, 0, "writing DIV resets it to 0");

        timer.tick(255);
        assert_eq!(timer.div, 0);
        timer.tick(1);
        assert_eq!(timer.div, 1, "divider restarts from zero after reset");
    }

    #[test]
    fn reports_apu_frame_events_on_divider_boundaries() {
        let mut timer = Timer::new();
        assert_eq!(timer.tick(8_191), 0);
        assert_eq!(timer.tick(1), 1);
        assert_eq!(timer.tick(16_384), 2);
    }

    #[test]
    fn tima_disabled_does_not_increment() {
        let mut timer = Timer::new();
        timer.write_register(TAC_ADDR, 0x00); // disabled
        timer.write_register(TIMA_ADDR, 0x10);
        timer.tick(8192);
        assert_eq!(timer.tima, 0x10);
        assert!(!timer.interrupt_pending());
    }

    #[test]
    fn tima_frequencies() {
        // TAC bits 0-1: 00 = /1024, 01 = /16, 10 = /64, 11 = /256.
        for (tac, period) in [(0x04u8, 1024u32), (0x05, 16), (0x06, 64), (0x07, 256)] {
            let mut timer = Timer::new();
            timer.write_register(TAC_ADDR, tac);
            timer.write_register(TIMA_ADDR, 0);
            timer.tick(period - 1);
            assert_eq!(timer.tima, 0, "TAC {tac:#04X}: no tick before period");
            timer.tick(1);
            assert_eq!(timer.tima, 1, "TAC {tac:#04X}: tick on period");
        }
    }

    #[test]
    fn tima_overflow_reloads_tma_and_requests_interrupt() {
        let mut timer = Timer::new();
        timer.write_register(TAC_ADDR, 0x05); // 16-cycle period
        timer.write_register(TMA_ADDR, 0x42);
        timer.write_register(TIMA_ADDR, 0xFF);

        timer.tick(16);
        assert_eq!(timer.tima, 0x42, "TIMA reloads from TMA on overflow");
        assert!(
            timer.take_interrupt(),
            "overflow raises the timer interrupt"
        );
        assert!(!timer.take_interrupt(), "interrupt is consumed once");
    }

    #[test]
    fn tac_unused_bits_read_as_one() {
        let mut timer = Timer::new();
        timer.write_register(TAC_ADDR, 0x05);
        assert_eq!(timer.read_register(TAC_ADDR), 0xFD);
    }

    #[test]
    fn mmu_routes_timer_registers() {
        let mut mmu = Mmu::new();
        mmu.write_byte(DIV_ADDR, 0x12);
        assert_eq!(
            mmu.read_byte(DIV_ADDR),
            0x00,
            "DIV always reads the counter"
        );

        mmu.write_byte(TIMA_ADDR, 0x34);
        assert_eq!(mmu.read_byte(TIMA_ADDR), 0x34);

        mmu.write_byte(TMA_ADDR, 0x56);
        assert_eq!(mmu.read_byte(TMA_ADDR), 0x56);

        mmu.write_byte(TAC_ADDR, 0x05);
        assert_eq!(mmu.read_byte(TAC_ADDR), 0xFD);
    }

    #[test]
    fn mmu_tick_raises_timer_interrupt_flag() {
        let mut mmu = Mmu::new();
        mmu.write_byte(TAC_ADDR, 0x05); // 16-cycle period
        mmu.write_byte(TMA_ADDR, 0x00);
        mmu.write_byte(TIMA_ADDR, 0xFF);
        mmu.write_byte(IF_ADDR, 0x00);

        mmu.tick(16);

        assert_eq!(mmu.read_byte(TIMA_ADDR), 0x00);
        assert_eq!(mmu.read_byte(IF_ADDR) & 0x04, 0x04, "IF bit 2 is set");
    }

    #[test]
    fn mmu_div_increments_from_cpu_ticks() {
        let mut mmu = Mmu::new();
        mmu.tick(256);
        assert_eq!(mmu.read_byte(DIV_ADDR), 1);
        mmu.tick(256);
        assert_eq!(mmu.read_byte(DIV_ADDR), 2);
    }
}
