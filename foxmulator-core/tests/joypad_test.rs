//! Joypad/P1 tests derived from Pan Docs (active-low, group selected by bits
//! 4-5 of the register).

#[cfg(test)]
mod joypad_tests {
    use foxmulator_core::joypad::{Button, Joypad};
    use foxmulator_core::mmu::{Mmu, IF_ADDR, JOYP_ADDR};

    #[test]
    fn released_state_reads_all_lines_high() {
        let joypad = Joypad::new();
        // Both groups deselected, low nibble all 1 (no button held).
        assert_eq!(joypad.read() & 0x0F, 0x0F);
    }

    #[test]
    fn direction_buttons_are_active_low_when_selected() {
        let mut joypad = Joypad::new();
        joypad.write(0x20); // bit 4 = 0: direction group selected, action deselected
        joypad.set_button(Button::Right, true);
        assert_eq!(joypad.read() & 0x0F, 0x0E, "Right pulls bit 0 low");

        joypad.set_button(Button::Down, true);
        assert_eq!(joypad.read() & 0x0F, 0x06, "Down pulls bit 3 low");
    }

    #[test]
    fn action_buttons_are_hidden_while_direction_selected() {
        let mut joypad = Joypad::new();
        joypad.write(0x20); // direction selected
        joypad.set_button(Button::A, true);
        assert_eq!(joypad.read() & 0x0F, 0x0F, "A is in the other group");
    }

    #[test]
    fn action_group_selection() {
        let mut joypad = Joypad::new();
        joypad.write(0x10); // action group selected
        joypad.set_button(Button::A, true);
        joypad.set_button(Button::Start, true);
        // A = bit 0, Start = bit 3, active-low -> 0b0110.
        assert_eq!(joypad.read() & 0x0F, 0b0110);
    }

    #[test]
    fn press_requests_interrupt_for_selected_group() {
        let mut joypad = Joypad::new();
        joypad.write(0x20); // select directions
        assert!(!joypad.set_button(Button::Left, false));
        assert!(joypad.set_button(Button::Left, true), "falling edge");
        assert!(!joypad.set_button(Button::Left, true), "no second edge");
        assert!(!joypad.set_button(Button::Left, false));
    }

    #[test]
    fn mmu_routes_joyp_and_sets_if_bit_4() {
        let mut mmu = Mmu::new();
        mmu.write_byte(IF_ADDR, 0);
        mmu.write_byte(JOYP_ADDR, 0x20); // select directions
        assert_eq!(mmu.read_byte(JOYP_ADDR) & 0x0F, 0x0F);

        mmu.set_button(Button::Up, true);
        assert_eq!(mmu.read_byte(JOYP_ADDR) & 0x0F, 0b1011);
        assert_eq!(mmu.read_byte(IF_ADDR) & 0x10, 0x10);
    }

    #[test]
    fn write_that_selects_pressed_group_requests_interrupt() {
        let mut mmu = Mmu::new();
        mmu.write_byte(IF_ADDR, 0);
        mmu.set_button(Button::B, true); // press while no group selected
        assert_eq!(mmu.read_byte(IF_ADDR) & 0x10, 0x00);

        mmu.write_byte(JOYP_ADDR, 0x10); // select action group while B is held
        assert_eq!(mmu.read_byte(JOYP_ADDR) & 0x0F, 0b1101);
        assert_eq!(mmu.read_byte(IF_ADDR) & 0x10, 0x10);
    }
}
