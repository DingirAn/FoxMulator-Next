//! Interrupt controller and vector tests derived from Pan Docs, "Interrupts".

#[cfg(test)]
mod tests {
    use foxmulator_core::interrupts::{
        highest_priority, pending_vector, vector_for_bit, InterruptController, ALL, JOYPAD,
        LCD_STAT, SERIAL, TIMER, VBLANK,
    };

    #[test]
    fn interrupt_controller() {
        let mut ic = InterruptController::new();
        assert!(!ic.pending());
        ic.enable(VBLANK);
        ic.set_flag(VBLANK);
        assert!(ic.pending());
        ic.clear_flag(VBLANK);
        assert!(!ic.pending());
    }

    #[test]
    fn vectors_match_pan_docs() {
        assert_eq!(vector_for_bit(0), Some(0x0040));
        assert_eq!(vector_for_bit(1), Some(0x0048));
        assert_eq!(vector_for_bit(2), Some(0x0050));
        assert_eq!(vector_for_bit(3), Some(0x0058));
        assert_eq!(vector_for_bit(4), Some(0x0060));
        assert_eq!(vector_for_bit(5), None);
    }

    #[test]
    fn highest_priority_wins() {
        // Timer (bit 2) and VBlank (bit 0) pending: VBlank has priority.
        assert_eq!(highest_priority(VBLANK | TIMER, ALL), Some(0));
        // Only joypad enabled.
        assert_eq!(highest_priority(JOYPAD, JOYPAD), Some(4));
    }

    #[test]
    fn disabled_interrupts_are_ignored() {
        assert_eq!(highest_priority(VBLANK, TIMER), None);
        assert_eq!(pending_vector(VBLANK, 0), None);
    }

    #[test]
    fn pending_vector_selects_lowest_bit() {
        let if_reg = SERIAL | LCD_STAT;
        assert_eq!(pending_vector(if_reg, ALL), Some(0x0048));
    }

    #[test]
    fn controller_masks_unknown_bits() {
        let mut ic = InterruptController::new();
        ic.enable(0xFF);
        ic.set_flag(0xFF);
        assert_eq!(ic.enabled, ALL);
        assert_eq!(ic.flag, ALL);
        assert_eq!(ic.pending_bit(), Some(0));
        assert_eq!(ic.pending_vector(), Some(0x0040));
    }
}
