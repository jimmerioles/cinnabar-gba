//! Audio Processing Unit.
//!
//! This is a register-complete stub: games can configure sound without
//! crashing, and the frontend gets a stable sample queue. The real channel
//! synthesis lives behind the same interface and is the next audio milestone.

pub struct Apu {
    regs: [u8; 0x48],
    sample_rate: u32,
    samples: Vec<f32>,
}

impl Default for Apu {
    fn default() -> Self {
        Self::new()
    }
}

impl Apu {
    pub fn new() -> Self {
        Self {
            regs: [0; 0x48],
            sample_rate: 48_000,
            samples: Vec::with_capacity(2048),
        }
    }

    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    pub fn read16(&self, offset: u16) -> u16 {
        let offset = (offset as usize & !1).min(self.regs.len() - 2);
        u16::from_le_bytes([self.regs[offset], self.regs[offset + 1]])
    }

    pub fn write16(&mut self, offset: u16, value: u16) {
        let offset = (offset as usize & !1).min(self.regs.len() - 2);
        let bytes = value.to_le_bytes();
        self.regs[offset] = bytes[0];
        self.regs[offset + 1] = bytes[1];
    }

    pub fn step(&mut self, _cycles: u32) {}

    pub fn take_samples(&mut self) -> Vec<f32> {
        std::mem::take(&mut self.samples)
    }
}
