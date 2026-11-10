//! Cartridge header parsing and ROM access.

use serde::{Deserialize, Serialize};

pub const ROM_BASE: u32 = 0x0800_0000;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SaveType {
    None,
    Sram,
    Flash64K,
    Flash128K,
    Eeprom512,
    Eeprom8K,
}

impl SaveType {
    pub fn size(self) -> usize {
        match self {
            Self::None => 0,
            Self::Sram => 32 * 1024,
            Self::Flash64K => 64 * 1024,
            Self::Flash128K => 128 * 1024,
            Self::Eeprom512 => 512,
            Self::Eeprom8K => 8 * 1024,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Sram => "SRAM 32K",
            Self::Flash64K => "Flash 64K",
            Self::Flash128K => "Flash 128K",
            Self::Eeprom512 => "EEPROM 512B",
            Self::Eeprom8K => "EEPROM 8K",
        }
    }
}

#[derive(Clone, Debug)]
pub struct Cartridge {
    pub rom: Vec<u8>,
    pub title: String,
    pub game_code: String,
    pub maker_code: String,
    pub save_type: SaveType,
    pub header_ok: bool,
}

impl Cartridge {
    pub fn new(rom: Vec<u8>) -> Self {
        let mut cart = Self {
            title: String::new(),
            game_code: String::new(),
            maker_code: String::new(),
            save_type: SaveType::None,
            header_ok: false,
            rom,
        };
        cart.parse_header();
        cart
    }

    fn parse_header(&mut self) {
        if self.rom.len() >= 0xB0 {
            self.title = ascii_field(&self.rom[0xA0..0xAC]);
            self.game_code = ascii_field(&self.rom[0xAC..0xB0]);
        }
        if self.rom.len() >= 0xB2 {
            self.maker_code = ascii_field(&self.rom[0xB0..0xB2]);
        }
        self.header_ok = self.rom.len() >= 0xC0 && self.complement_ok();
        self.save_type = detect_save_type(&self.rom);
    }

    fn complement_ok(&self) -> bool {
        if self.rom.len() < 0xBE {
            return false;
        }
        let sum: u8 = self.rom[0xA0..=0xBC]
            .iter()
            .fold(0u8, |acc, &b| acc.wrapping_add(b));
        let expected = 0u8.wrapping_sub(sum).wrapping_sub(0x19);
        self.rom[0xBD] == expected
    }

    fn offset(addr: u32) -> usize {
        // The cartridge window is 32 MiB and mirrors every 32 MiB.
        (addr.wrapping_sub(ROM_BASE) & 0x01FF_FFFF) as usize
    }

    pub fn read8(&self, addr: u32) -> u8 {
        let offset = Self::offset(addr);
        self.rom.get(offset).copied().unwrap_or(0)
    }

    pub fn read16(&self, addr: u32) -> u16 {
        let offset = Self::offset(addr);
        match (self.rom.get(offset), self.rom.get(offset + 1)) {
            (Some(&lo), Some(&hi)) => u16::from_le_bytes([lo, hi]),
            (Some(&lo), None) => lo as u16,
            _ => 0,
        }
    }

    pub fn read32(&self, addr: u32) -> u32 {
        let offset = Self::offset(addr);
        match (
            self.rom.get(offset),
            self.rom.get(offset + 1),
            self.rom.get(offset + 2),
            self.rom.get(offset + 3),
        ) {
            (Some(&a), Some(&b), Some(&c), Some(&d)) => u32::from_le_bytes([a, b, c, d]),
            _ => 0,
        }
    }

    pub fn size(&self) -> usize {
        self.rom.len()
    }
}

fn ascii_field(bytes: &[u8]) -> String {
    let end = bytes
        .iter()
        .position(|&b| b == 0 || b == 0xFF)
        .unwrap_or(bytes.len());
    String::from_utf8_lossy(&bytes[..end]).trim().to_string()
}

fn detect_save_type(rom: &[u8]) -> SaveType {
    let has = |needle: &[u8]| rom.windows(needle.len()).any(|w| w == needle);

    if has(b"EEPROM_V") {
        // Most commercial EEPROM games use 8 KiB. 512-byte games are
        // detected by their save access pattern at runtime.
        SaveType::Eeprom8K
    } else if has(b"FLASH1M_V") {
        SaveType::Flash128K
    } else if has(b"FLASH512_V") || has(b"FLASH_V") {
        SaveType::Flash64K
    } else if has(b"SRAM_V") {
        SaveType::Sram
    } else {
        SaveType::None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn header_rom() -> Vec<u8> {
        let mut rom = vec![0u8; 0x1000];
        rom[0xA0..0xA4].copy_from_slice(b"TEST");
        rom[0xAC..0xB0].copy_from_slice(b"ABCD");
        rom[0xB0..0xB2].copy_from_slice(b"01");
        let sum: u8 = rom[0xA0..=0xBC]
            .iter()
            .fold(0u8, |acc, &b| acc.wrapping_add(b));
        rom[0xBD] = 0u8.wrapping_sub(sum).wrapping_sub(0x19);
        rom
    }

    #[test]
    fn parses_header() {
        let cart = Cartridge::new(header_rom());
        assert_eq!(cart.title, "TEST");
        assert_eq!(cart.game_code, "ABCD");
        assert_eq!(cart.maker_code, "01");
        assert!(cart.header_ok);
    }

    #[test]
    fn detects_sram() {
        let mut rom = header_rom();
        rom[0x200..0x206].copy_from_slice(b"SRAM_V");
        let cart = Cartridge::new(rom);
        assert_eq!(cart.save_type, SaveType::Sram);
    }

    #[test]
    fn detects_eeprom() {
        let mut rom = header_rom();
        rom[0x200..0x208].copy_from_slice(b"EEPROM_V");
        let cart = Cartridge::new(rom);
        assert_eq!(cart.save_type, SaveType::Eeprom8K);
    }
}
