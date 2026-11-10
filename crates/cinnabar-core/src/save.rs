//! Battery-backed save media.
//!
//! SRAM is a straightforward byte-addressable window. Flash and EEPROM use
//! command protocols on real hardware; Cinnabar currently exposes them as
//! byte-addressable storage and keeps the door open for proper state machines
//! once game compatibility work begins.

use crate::cart::SaveType;

#[derive(Clone, Debug)]
pub struct SaveMedia {
    pub kind: SaveType,
    pub data: Vec<u8>,
    pub dirty: bool,
}

impl SaveMedia {
    pub fn new(kind: SaveType) -> Self {
        let fill = if kind == SaveType::None { 0 } else { 0xFF };
        Self {
            kind,
            data: vec![fill; kind.size()],
            dirty: false,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    pub fn read8(&mut self, addr: u32) -> u8 {
        if self.data.is_empty() {
            return 0xFF;
        }
        let offset = (addr as usize) & 0x1FFFF;
        self.data[offset % self.data.len()]
    }

    pub fn write8(&mut self, addr: u32, value: u8) {
        if self.data.is_empty() {
            return;
        }
        let offset = (addr as usize) & 0x1FFFF;
        let index = offset % self.data.len();
        if self.data[index] != value {
            self.data[index] = value;
            self.dirty = true;
        }
    }

    pub fn load(&mut self, bytes: &[u8]) {
        if self.data.is_empty() {
            return;
        }
        let count = bytes.len().min(self.data.len());
        self.data[..count].copy_from_slice(&bytes[..count]);
        self.dirty = false;
    }

    pub fn bytes(&self) -> &[u8] {
        &self.data
    }

    pub fn mark_clean(&mut self) {
        self.dirty = false;
    }
}
