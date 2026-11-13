//! Boot ROMs distributed with FoxMulator.
//!
//! These are SameBoy SameBoot binaries compiled from the pinned source tree.
//! Keeping them as `include_bytes!` resources makes the core self-contained and
//! avoids any runtime firmware search or filesystem dependency.

pub const DMG: &[u8; 0x100] = include_bytes!("../resources/bootroms/dmg_boot.bin");
pub const CGB: &[u8; 0x900] = include_bytes!("../resources/bootroms/cgb_boot.bin");

pub fn for_model(cgb: bool) -> &'static [u8] {
    if cgb {
        CGB
    } else {
        DMG
    }
}
