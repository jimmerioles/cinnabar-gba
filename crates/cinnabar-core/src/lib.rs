//! Cinnabar: a clean-room Game Boy Advance emulator core.
//!
//! The core is deliberately headless: it owns no windows, no audio devices,
//! and no filesystem state. Everything is driven by [`System::run_frame`].

pub mod apu;
pub mod bios;
pub mod bus;
pub mod cart;
pub mod cpu;
pub mod disasm;
pub mod dma;
pub mod irq;
pub mod keypad;
pub mod ppu;
pub mod save;
pub mod system;
pub mod timer;

pub use cart::{Cartridge, SaveType};
pub use system::{FrameOutput, System};

/// Width of the GBA LCD in pixels.
pub const SCREEN_WIDTH: usize = 240;
/// Height of the GBA LCD in pixels.
pub const SCREEN_HEIGHT: usize = 160;
/// CPU cycles in one frame (228 scanlines × 1232 cycles).
pub const CYCLES_PER_FRAME: u32 = 280_896;
/// Cycles per scanline.
pub const CYCLES_PER_SCANLINE: u32 = 1_232;
/// Total scanlines per frame, including VBlank.
pub const SCANLINES: u16 = 228;
/// Visible scanlines.
pub const VISIBLE_SCANLINES: u16 = 160;
