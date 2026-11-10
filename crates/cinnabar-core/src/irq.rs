//! Interrupt controller (IE / IF / IME).

/// Interrupt bits, matching the GBA's IE/IF layout.
use serde::{Deserialize, Serialize};

pub mod bits {
    pub const VBLANK: u16 = 1 << 0;
    pub const HBLANK: u16 = 1 << 1;
    pub const VCOUNT: u16 = 1 << 2;
    pub const TIMER0: u16 = 1 << 3;
    pub const TIMER1: u16 = 1 << 4;
    pub const TIMER2: u16 = 1 << 5;
    pub const TIMER3: u16 = 1 << 6;
    pub const DMA0: u16 = 1 << 8;
    pub const DMA1: u16 = 1 << 9;
    pub const DMA2: u16 = 1 << 10;
    pub const DMA3: u16 = 1 << 11;
    pub const KEYPAD: u16 = 1 << 12;
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Irq {
    pub ie: u16,
    pub if_: u16,
    pub ime: bool,
}

impl Irq {
    pub fn request(&mut self, bits: u16) {
        self.if_ |= bits & 0x3FFF;
    }

    pub fn pending(&self) -> bool {
        self.ime && (self.ie & self.if_ & 0x3FFF) != 0
    }

    pub fn acknowledge(&mut self, bits: u16) {
        self.if_ &= !(bits & 0x3FFF);
    }

    pub fn clear(&mut self) {
        self.ie = 0;
        self.if_ = 0;
        self.ime = false;
    }
}
