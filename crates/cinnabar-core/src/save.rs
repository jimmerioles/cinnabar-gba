//! Battery-backed save media.
//!
//! SRAM and Flash sit on an 8-bit bus. Byte lanes matter: a 16-bit store
//! writes only the addressed byte, and wide reads replicate the addressed
//! byte. That sounds odd until you read the gba-tests expectations.
//!
//! Flash also implements the AMD command set used by GBA cartridges:
//! unlock (AA/55), program (A0), chip erase (80/10), sector erase (80/30)
//! and 128 KiB bank select (B0).

use crate::cart::SaveType;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum FlashState {
    Read,
    Unlock1,
    Unlock2,
    Program,
    BankSelect,
    EraseUnlock1,
    EraseUnlock2,
    EraseUnlock3,
}

#[derive(Clone, Debug)]
pub struct SaveMedia {
    pub kind: SaveType,
    pub data: Vec<u8>,
    pub dirty: bool,
    flash_state: FlashState,
    flash_bank: usize,
}

impl SaveMedia {
    pub fn new(kind: SaveType) -> Self {
        let fill = if kind == SaveType::None { 0 } else { 0xFF };
        Self {
            kind,
            data: vec![fill; kind.size()],
            dirty: false,
            flash_state: FlashState::Read,
            flash_bank: 0,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    fn is_flash(&self) -> bool {
        matches!(self.kind, SaveType::Flash64K | SaveType::Flash128K)
    }

    fn data_offset(&self, addr: u32) -> usize {
        let len = self.data.len();
        if len == 0 {
            return 0;
        }
        if self.is_flash() {
            let bank_size = 0x1_0000;
            let banks = (len / bank_size).max(1);
            let bank = self.flash_bank % banks;
            (bank * bank_size + (addr as usize & 0xFFFF)) % len
        } else {
            (addr as usize & 0x1FFFF) % len
        }
    }

    pub fn read8(&mut self, addr: u32) -> u8 {
        if self.data.is_empty() {
            return 0xFF;
        }
        let offset = self.data_offset(addr);
        self.data[offset]
    }

    pub fn read16(&mut self, addr: u32) -> u16 {
        let byte = self.read8(addr);
        byte as u16 | ((byte as u16) << 8)
    }

    pub fn read32(&mut self, addr: u32) -> u32 {
        let byte = self.read8(addr) as u32;
        byte | (byte << 8) | (byte << 16) | (byte << 24)
    }

    pub fn write8(&mut self, addr: u32, value: u8) {
        if self.data.is_empty() {
            return;
        }
        if self.is_flash() {
            self.flash_write8(addr, value);
        } else {
            self.write_byte(addr, value);
        }
    }

    pub fn write16(&mut self, addr: u32, value: u16) {
        if self.data.is_empty() {
            return;
        }
        if self.is_flash() {
            if self.flash_state == FlashState::Program {
                let byte = if addr & 1 == 0 {
                    value as u8
                } else {
                    (value >> 8) as u8
                };
                self.write_byte(addr, byte);
                self.flash_state = FlashState::Read;
            }
        } else {
            let byte = if addr & 1 == 0 {
                value as u8
            } else {
                (value >> 8) as u8
            };
            self.write_byte(addr, byte);
        }
    }

    pub fn write32(&mut self, addr: u32, value: u32) {
        if self.data.is_empty() {
            return;
        }
        if self.is_flash() {
            if self.flash_state == FlashState::Program {
                let byte = (value >> ((addr & 3) * 8)) as u8;
                self.write_byte(addr, byte);
                self.flash_state = FlashState::Read;
            }
        } else {
            let byte = (value >> ((addr & 3) * 8)) as u8;
            self.write_byte(addr, byte);
        }
    }

    fn write_byte(&mut self, addr: u32, value: u8) {
        let offset = self.data_offset(addr);
        if self.data[offset] != value {
            self.data[offset] = value;
            self.dirty = true;
        }
    }

    fn flash_write8(&mut self, addr: u32, value: u8) {
        let low = (addr & 0xFFFF) as u16;
        match self.flash_state {
            FlashState::Read => {
                if low == 0x5555 && value == 0xAA {
                    self.flash_state = FlashState::Unlock1;
                }
            }
            FlashState::Unlock1 => {
                self.flash_state = if low == 0x2AAA && value == 0x55 {
                    FlashState::Unlock2
                } else {
                    FlashState::Read
                };
            }
            FlashState::Unlock2 => {
                self.flash_state = if low != 0x5555 {
                    FlashState::Read
                } else {
                    match value {
                        0xA0 => FlashState::Program,
                        0x80 => FlashState::EraseUnlock1,
                        0xB0 => FlashState::BankSelect,
                        0x90 | 0xF0 => FlashState::Read,
                        _ => FlashState::Read,
                    }
                };
            }
            FlashState::Program => {
                self.write_byte(addr, value);
                self.flash_state = FlashState::Read;
            }
            FlashState::BankSelect => {
                if low == 0 {
                    self.flash_bank = (value & 1) as usize;
                }
                self.flash_state = FlashState::Read;
            }
            FlashState::EraseUnlock1 => {
                self.flash_state = if low == 0x5555 && value == 0xAA {
                    FlashState::EraseUnlock2
                } else {
                    FlashState::Read
                };
            }
            FlashState::EraseUnlock2 => {
                self.flash_state = if low == 0x2AAA && value == 0x55 {
                    FlashState::EraseUnlock3
                } else {
                    FlashState::Read
                };
            }
            FlashState::EraseUnlock3 => {
                match value {
                    0x10 => self.chip_erase(),
                    0x30 => self.sector_erase(addr),
                    _ => {}
                }
                self.flash_state = FlashState::Read;
            }
        }
    }

    fn chip_erase(&mut self) {
        self.data.fill(0xFF);
        self.dirty = true;
    }

    fn sector_erase(&mut self, addr: u32) {
        let start = self.data_offset(addr) & !0xFFF;
        let end = (start + 0x1000).min(self.data.len());
        self.data[start..end].fill(0xFF);
        self.dirty = true;
    }

    pub fn load(&mut self, bytes: &[u8]) {
        if self.data.is_empty() {
            return;
        }
        let count = bytes.len().min(self.data.len());
        self.data[..count].copy_from_slice(&bytes[..count]);
        self.dirty = false;
        self.flash_state = FlashState::Read;
        self.flash_bank = 0;
    }

    pub fn bytes(&self) -> &[u8] {
        &self.data
    }

    pub fn mark_clean(&mut self) {
        self.dirty = false;
    }
}
