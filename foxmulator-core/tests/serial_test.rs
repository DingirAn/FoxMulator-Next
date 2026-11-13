//! Serial port tests derived from Pan Docs, "Serial Data Transfer (Link Cable)".

#[cfg(test)]
mod tests {
    use foxmulator_core::mmu::Mmu;
    use foxmulator_core::serial::{Serial, SB_ADDR, SC_ADDR, TRANSFER_CYCLES};
    use foxmulator_core::system::{LinkCable, System};

    const IF_ADDR: u16 = 0xFF0F;

    #[test]
    fn legacy_link_queue_api_still_works() {
        let mut serial = Serial::new();
        serial.write(0xAB);
        // Since we have no DMA, the byte should be queued for transmission
        assert_eq!(serial.tx_len(), 1);
        // No receive yet
        assert!(serial.read().is_none());
    }

    #[test]
    fn sc_unused_bits_read_as_one() {
        let serial = Serial::new();
        assert_eq!(serial.read_register(SC_ADDR), 0x7E, "SC bits 1-6 read 1");
    }

    #[test]
    fn internal_transfer_completes_after_4096_cycles() {
        let mut serial = Serial::new();
        serial.write_register(SB_ADDR, 0x42);
        serial.write_register(SC_ADDR, 0x81);

        assert!(serial.busy());
        assert!(!serial.tick(TRANSFER_CYCLES - 1), "not complete yet");
        assert!(serial.busy());

        assert!(serial.tick(1), "completes on the 4096th cycle");
        assert!(!serial.busy());
        assert_eq!(
            serial.read_register(SC_ADDR) & 0x80,
            0x00,
            "SC bit 7 clears"
        );
        assert_eq!(serial.read_register(SB_ADDR), 0xFF, "no partner -> 0xFF");
        assert_eq!(serial.tx_len(), 1, "sent byte is queued for the link");
    }

    #[test]
    fn external_clock_transfer_does_not_advance() {
        let mut serial = Serial::new();
        serial.write_register(SB_ADDR, 0xA6);
        serial.write_register(SC_ADDR, 0x80); // start with external clock
        assert!(serial.busy());
        assert!(!serial.tick(TRANSFER_CYCLES * 4));
        assert!(serial.busy());
        assert!(serial.clock_external(0x5C));
        assert!(!serial.busy());
        assert_eq!(serial.read_register(SB_ADDR), 0x5C);
        assert_eq!(serial.take_tx(), vec![0xA6]);
    }

    #[test]
    fn mmu_external_clock_completes_and_raises_interrupt() {
        let mut mmu = Mmu::new();
        mmu.write_byte(IF_ADDR, 0);
        mmu.write_byte(SB_ADDR, 0x91);
        mmu.write_byte(SC_ADDR, 0x80);
        assert!(mmu.serial.busy());
        assert!(mmu.clock_external_serial(0x2E));
        assert_eq!(mmu.read_byte(SB_ADDR), 0x2E);
        assert_eq!(mmu.read_byte(IF_ADDR) & 0x08, 0x08);
        assert!(!mmu.serial.busy());
    }

    #[test]
    fn link_partner_byte_is_received() {
        let mut serial = Serial::new();
        serial.receive(0x5A);
        serial.write_register(SB_ADDR, 0x11);
        serial.write_register(SC_ADDR, 0x81);
        assert!(serial.tick(TRANSFER_CYCLES));

        assert_eq!(serial.read_register(SB_ADDR), 0x5A);
        assert_eq!(serial.rx_len(), 0);
    }

    #[test]
    fn mmu_routes_sb_and_sc_and_raises_if_bit_3() {
        let mut mmu = Mmu::new();
        mmu.write_byte(IF_ADDR, 0);
        mmu.write_byte(SB_ADDR, 0xA5);
        assert_eq!(mmu.read_byte(SB_ADDR), 0xA5);

        mmu.write_byte(SC_ADDR, 0x81);
        mmu.tick(TRANSFER_CYCLES);
        assert_eq!(mmu.read_byte(IF_ADDR) & 0x08, 0x08, "serial IF bit");
        assert_eq!(mmu.read_byte(SB_ADDR), 0xFF);
    }

    #[test]
    fn virtual_link_exchanges_two_independent_internal_clock_transfers() {
        let mut left = System::new(false);
        let mut right = System::new(false);
        left.mmu.write_byte(SB_ADDR, 0xA5);
        left.mmu.write_byte(SC_ADDR, 0x81);
        right.mmu.write_byte(SB_ADDR, 0x3C);
        right.mmu.write_byte(SC_ADDR, 0x81);

        let mut cable = LinkCable::new();
        cable.tick(&mut left, &mut right, TRANSFER_CYCLES);

        assert_eq!(left.mmu.read_byte(SB_ADDR), 0x3C);
        assert_eq!(right.mmu.read_byte(SB_ADDR), 0xA5);
        assert_ne!(left.mmu.read_byte(IF_ADDR) & 0x08, 0);
        assert_ne!(right.mmu.read_byte(IF_ADDR) & 0x08, 0);
        assert!(!left.mmu.serial.busy());
        assert!(!right.mmu.serial.busy());
    }

    #[test]
    fn virtual_link_keeps_consoles_independent_when_only_one_transmits() {
        let mut left = System::new(false);
        let mut right = System::new(false);
        left.mmu.write_byte(SB_ADDR, 0xD2);
        left.mmu.write_byte(SC_ADDR, 0x81);

        let mut cable = LinkCable::new();
        cable.tick(&mut left, &mut right, TRANSFER_CYCLES);

        assert_eq!(left.mmu.read_byte(SB_ADDR), 0xFF);
        assert_eq!(right.mmu.read_byte(SB_ADDR), 0x00);
        assert_ne!(left.mmu.read_byte(IF_ADDR) & 0x08, 0);
        assert_eq!(right.mmu.read_byte(IF_ADDR) & 0x08, 0);
    }

    #[test]
    fn virtual_link_supports_one_internal_and_one_external_clock() {
        let mut left = System::new(false);
        let mut right = System::new(false);
        left.mmu.write_byte(SB_ADDR, 0xA5);
        left.mmu.write_byte(SC_ADDR, 0x81); // left supplies the clock
        right.mmu.write_byte(SB_ADDR, 0x3C);
        right.mmu.write_byte(SC_ADDR, 0x80); // right receives the clock

        let mut cable = LinkCable::new();
        cable.tick(&mut left, &mut right, TRANSFER_CYCLES);

        assert_eq!(left.mmu.read_byte(SB_ADDR), 0x3C);
        assert_eq!(right.mmu.read_byte(SB_ADDR), 0xA5);
        assert_ne!(left.mmu.read_byte(IF_ADDR) & 0x08, 0);
        assert_ne!(right.mmu.read_byte(IF_ADDR) & 0x08, 0);
        assert!(!left.mmu.serial.busy());
        assert!(!right.mmu.serial.busy());
    }
}
