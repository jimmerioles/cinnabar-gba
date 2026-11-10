//! Keypad input.

/// Button bits, active-low in the hardware register.
pub mod keys {
    pub const A: u16 = 1 << 0;
    pub const B: u16 = 1 << 1;
    pub const SELECT: u16 = 1 << 2;
    pub const START: u16 = 1 << 3;
    pub const RIGHT: u16 = 1 << 4;
    pub const LEFT: u16 = 1 << 5;
    pub const UP: u16 = 1 << 6;
    pub const DOWN: u16 = 1 << 7;
    pub const R: u16 = 1 << 8;
    pub const L: u16 = 1 << 9;
}

#[derive(Clone, Debug, Default)]
pub struct Keypad {
    /// Bit set = pressed.
    pub pressed: u16,
    pub keycnt: u16,
}

impl Keypad {
    pub fn set_pressed(&mut self, pressed: u16) {
        self.pressed = pressed & 0x3FF;
    }

    pub fn read(&self) -> u16 {
        (!self.pressed) & 0x3FF
    }

    /// Evaluate the KEYCNT IRQ condition.
    pub fn irq_condition(&self) -> bool {
        if self.keycnt & 0x4000 == 0 {
            return false;
        }
        let mask = self.keycnt & 0x3FF;
        if mask == 0 {
            return false;
        }
        let pressed = self.pressed & mask;
        if self.keycnt & 0x8000 != 0 {
            pressed == mask
        } else {
            pressed != 0
        }
    }
}
