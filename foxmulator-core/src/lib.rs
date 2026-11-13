//! FoxMulator Core — Núcleo de emulación Game Boy / Game Boy Color
pub mod apu;
mod bootrom;
pub mod cartridge;
pub mod cpu;
pub mod interrupts;
pub mod joypad;
pub mod mbc;
pub mod mmu;
pub mod ppu;
pub mod serial;
pub mod system;
pub mod timer;
pub mod timer_mod;

pub use cartridge::{Cartridge, MbcType, RomType};
pub use cpu::Cpu;
pub use mmu::Mmu;
pub use system::{LinkCable, System};

/// Dimensiones nativas de la pantalla Game Boy
pub const SCREEN_WIDTH: u32 = 160;
pub const SCREEN_HEIGHT: u32 = 144;
pub const PIXEL_BUFFER_SIZE: usize = (SCREEN_WIDTH * SCREEN_HEIGHT) as usize;

/// Framebuffer en formato RGBA8888 para renderizado
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Pixel(pub u32);

impl Default for Pixel {
    fn default() -> Self {
        Pixel(0xFF1E8C40)
    } // DMG dark green background
}

/// Estado de emulación para pausa/reanudar
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub enum EmuState {
    #[default]
    Running,
    Paused,
    Stopped,
}
