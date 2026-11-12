//! Battery-backed save media.
//!
//! SRAM and Flash sit on an 8-bit bus. Byte lanes matter: a 16-bit store
//! writes only the addressed byte, and wide reads replicate the addressed
//! byte. That sounds odd until you read the gba-tests expectations.
//!
//! Flash also implements the AMD command set used by GBA cartridges:
//! unlock (AA/55), program (A0), chip erase (80/10), sector erase (80/30)
//! and 128 KiB bank select (B0).

use serde::{Deserialize, Serialize};

use crate::cart::SaveType;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
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

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
enum EepromPhase {
    #[default]
    Idle,
    Address,
    Read,
    Write,
    WriteTail,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
struct EepromState {
    phase: EepromPhase,
    command: u8,
    command_bits: u8,
    read: bool,
    address: u16,
    address_bits: u8,
    expected_address_bits: u8,
    data: u64,
    data_bits: u8,
    output_bits: u8,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SaveMedia {
    pub kind: SaveType,
    pub data: Vec<u8>,
    pub dirty: bool,
    flash_state: FlashState,
    flash_bank: usize,
    flash_id_mode: bool,
    eeprom: EepromState,
}

impl SaveMedia {
    pub fn new(kind: SaveType) -> Self {
        let fill = if kind == SaveType::None { 0 } else { 0xFF };
        let eeprom_bits = if kind == SaveType::Eeprom512 { 6 } else { 14 };
        Self {
            kind,
            data: vec![fill; kind.size()],
            dirty: false,
            flash_state: FlashState::Read,
            flash_bank: 0,
            flash_id_mode: false,
            eeprom: EepromState {
                expected_address_bits: eeprom_bits,
                ..EepromState::default()
            },
        }
    }

    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    fn is_flash(&self) -> bool {
        matches!(self.kind, SaveType::Flash64K | SaveType::Flash128K)
    }

    pub fn is_eeprom(&self) -> bool {
        matches!(self.kind, SaveType::Eeprom512 | SaveType::Eeprom8K)
    }

    /// Tell EEPROM how many transfers the next DMA will make. Games use a
    /// 9-halfword read for 512 B parts and a 17-halfword read for 8 KiB parts;
    /// write transfers use 73 and 81 halfwords respectively.
    pub fn eeprom_set_dma_length(&mut self, length: u32) {
        if !self.is_eeprom() {
            return;
        }
        let bits = match length {
            9 | 73 => 6,
            17 | 81 => 14,
            _ => return,
        };
        self.eeprom.expected_address_bits = bits;
        self.resize_eeprom(bits);
    }

    fn resize_eeprom(&mut self, address_bits: u8) {
        let (kind, size) = if address_bits == 6 {
            (SaveType::Eeprom512, SaveType::Eeprom512.size())
        } else {
            (SaveType::Eeprom8K, SaveType::Eeprom8K.size())
        };
        self.kind = kind;
        if self.data.len() < size {
            self.data.resize(size, 0xFF);
        } else if self.data.len() > size {
            self.data.truncate(size);
        }
    }

    pub fn eeprom_write_bit(&mut self, bit: bool) {
        if !self.is_eeprom() {
            return;
        }
        let bit = u8::from(bit);
        match self.eeprom.phase {
            EepromPhase::Idle => {
                self.eeprom.command = (self.eeprom.command << 1) | bit;
                self.eeprom.command_bits += 1;
                if self.eeprom.command_bits == 2 {
                    self.eeprom.command_bits = 0;
                    match self.eeprom.command {
                        0b11 => {
                            self.eeprom.read = true;
                            self.eeprom.phase = EepromPhase::Address;
                            self.eeprom.address = 0;
                            self.eeprom.address_bits = 0;
                            self.eeprom.output_bits = 0;
                        }
                        0b10 => {
                            self.eeprom.read = false;
                            self.eeprom.phase = EepromPhase::Address;
                            self.eeprom.address = 0;
                            self.eeprom.address_bits = 0;
                            self.eeprom.data = 0;
                            self.eeprom.data_bits = 0;
                        }
                        _ => {
                            // Write-disable / write-all commands are not
                            // needed for normal game saves.
                        }
                    }
                    self.eeprom.command = 0;
                }
            }
            EepromPhase::Address => {
                let expected = self.eeprom.expected_address_bits.max(6);
                if self.eeprom.address_bits < expected {
                    self.eeprom.address = (self.eeprom.address << 1) | u16::from(bit);
                    self.eeprom.address_bits += 1;
                } else if self.eeprom.read {
                    // The zero after the address closes a read request.
                    self.resize_eeprom(expected);
                    self.load_eeprom_block();
                    self.eeprom.phase = EepromPhase::Read;
                } else {
                    // A write request has no terminator: the next bit is
                    // already the first data bit.
                    self.resize_eeprom(expected);
                    self.eeprom.data = u64::from(bit);
                    self.eeprom.data_bits = 1;
                    self.eeprom.phase = EepromPhase::Write;
                }
            }
            EepromPhase::Write => {
                self.eeprom.data = (self.eeprom.data << 1) | u64::from(bit);
                self.eeprom.data_bits += 1;
                if self.eeprom.data_bits == 64 {
                    self.store_eeprom_block();
                    self.eeprom.phase = EepromPhase::WriteTail;
                }
            }
            EepromPhase::WriteTail => {
                // The final zero bit closes the write transfer.
                self.eeprom.phase = EepromPhase::Idle;
            }
            EepromPhase::Read => {
                // A write during a read reply starts a new command, matching
                // the chip-select edge. This keeps a truncated read from
                // swallowing the next request.
                self.eeprom.phase = EepromPhase::Idle;
                self.eeprom.command = bit;
                self.eeprom.command_bits = 1;
            }
        }
    }

    /// Read the next serial EEPROM bit. Four dummy bits precede the 64 data
    /// bits, matching the GBA's direct-sound-style DMA read sequence.
    pub fn eeprom_read_bit(&mut self) -> u8 {
        if !self.is_eeprom() {
            return 1;
        }
        let state = &mut self.eeprom;
        if state.phase != EepromPhase::Read {
            return 1;
        }
        state.output_bits += 1;
        if state.output_bits <= 4 {
            return 0;
        }
        if state.output_bits <= 68 {
            let shift = 68 - state.output_bits;
            let bit = ((state.data >> shift) & 1) as u8;
            if state.output_bits == 68 {
                // The transfer is complete after the 64th data bit. Returning
                // to idle here lets the next request start immediately; some
                // games read several blocks back-to-back without an extra
                // clock.
                state.phase = EepromPhase::Idle;
            }
            return bit;
        }
        state.phase = EepromPhase::Idle;
        1
    }

    fn eeprom_block_offset(&self) -> usize {
        let blocks = (self.data.len() / 8).max(1);
        let block = (self.eeprom.address as usize) % blocks;
        block * 8
    }

    fn load_eeprom_block(&mut self) {
        let offset = self.eeprom_block_offset();
        let mut bytes = [0u8; 8];
        for (index, byte) in bytes.iter_mut().enumerate() {
            *byte = self.data.get(offset + index).copied().unwrap_or(0xFF);
        }
        self.eeprom.data = u64::from_be_bytes(bytes);
    }

    fn store_eeprom_block(&mut self) {
        let offset = self.eeprom_block_offset();
        if offset + 8 <= self.data.len() {
            self.data[offset..offset + 8].copy_from_slice(&self.eeprom.data.to_be_bytes());
            self.dirty = true;
        }
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
        if self.is_flash() && self.flash_id_mode {
            return self.flash_id_byte(addr);
        }
        let offset = self.data_offset(addr);
        self.data[offset]
    }

    fn flash_id_byte(&self, addr: u32) -> u8 {
        // 64K parts report a Panasonic MN63F805MNP; 128K parts report a
        // Sanyo LE26FV10N1TS. The IDs are byte-swapped in the same way as
        // the 16-bit values used by hardware documentation.
        let (manufacturer, device) = if self.kind == SaveType::Flash128K {
            (0x62, 0x13)
        } else {
            (0x32, 0x1B)
        };
        match addr & 0xFFFF {
            0 => manufacturer,
            1 => device,
            _ => 0,
        }
    }

    /// Read a save byte without advancing the flash command state.
    pub fn peek8(&self, addr: u32) -> u8 {
        if self.data.is_empty() {
            return 0xFF;
        }
        if self.is_flash() && self.flash_id_mode {
            return self.flash_id_byte(addr);
        }
        self.data[self.data_offset(addr)]
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
            if self.flash_id_mode {
                if value == 0xF0 {
                    self.flash_id_mode = false;
                }
                return;
            }
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
            if self.flash_id_mode {
                let byte = if addr & 1 == 0 {
                    value as u8
                } else {
                    (value >> 8) as u8
                };
                self.write8(addr, byte);
            } else if self.flash_state == FlashState::Program {
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
            if self.flash_id_mode {
                let byte = (value >> ((addr & 3) * 8)) as u8;
                self.write8(addr, byte);
            } else if self.flash_state == FlashState::Program {
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
                        0x90 => {
                            self.flash_id_mode = true;
                            FlashState::Read
                        }
                        0xF0 => {
                            self.flash_id_mode = false;
                            FlashState::Read
                        }
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
        self.flash_id_mode = false;
    }

    pub fn bytes(&self) -> &[u8] {
        &self.data
    }

    pub fn mark_clean(&mut self) {
        self.dirty = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bus::{Bus, Memory};
    use crate::cart::Cartridge;

    fn send_bits(save: &mut SaveMedia, bits: &[u8]) {
        for bit in bits {
            save.eeprom_write_bit(*bit != 0);
        }
    }

    fn read_block(save: &mut SaveMedia, address_bits: u8, address: u16) -> u64 {
        let mut bits = vec![1, 1];
        for shift in (0..address_bits).rev() {
            bits.push(((address >> shift) & 1) as u8);
        }
        bits.push(0);
        send_bits(save, &bits);
        let mut value = 0u64;
        for index in 0..68 {
            let bit = save.eeprom_read_bit();
            if index >= 4 {
                value = (value << 1) | u64::from(bit);
            }
        }
        value
    }

    fn write_block(save: &mut SaveMedia, address_bits: u8, address: u16, value: u64) {
        let mut bits = vec![1, 0];
        for shift in (0..address_bits).rev() {
            bits.push(((address >> shift) & 1) as u8);
        }
        for shift in (0..64).rev() {
            bits.push(((value >> shift) & 1) as u8);
        }
        bits.push(0);
        send_bits(save, &bits);
    }

    fn flash_unlock(save: &mut SaveMedia, command: u8) {
        save.write8(0x0E00_5555, 0xAA);
        save.write8(0x0E00_2AAA, 0x55);
        save.write8(0x0E00_5555, command);
    }

    #[test]
    fn flash_id_mode_and_bank_select() {
        let mut save = SaveMedia::new(SaveType::Flash128K);
        flash_unlock(&mut save, 0x90);
        assert_eq!(save.read8(0x0E00_0000), 0x62);
        assert_eq!(save.read8(0x0E00_0001), 0x13);
        save.write8(0x0E00_0000, 0xF0);
        save.data[0] = 0x12;
        assert_eq!(save.read8(0x0E00_0000), 0x12);

        flash_unlock(&mut save, 0xB0);
        save.write8(0x0E00_0000, 1);
        save.data[0x1_0000] = 0x34;
        assert_eq!(save.read8(0x0E00_0000), 0x34);
        flash_unlock(&mut save, 0xB0);
        save.write8(0x0E00_0000, 0);
        assert_eq!(save.read8(0x0E00_0000), 0x12);
    }

    #[test]
    fn flash_64k_reports_the_panasonic_id() {
        let mut save = SaveMedia::new(SaveType::Flash64K);
        flash_unlock(&mut save, 0x90);
        assert_eq!(save.read8(0x0E00_0000), 0x32);
        assert_eq!(save.read8(0x0E00_0001), 0x1B);
    }

    #[test]
    fn flash_program_and_sector_erase() {
        let mut save = SaveMedia::new(SaveType::Flash64K);
        flash_unlock(&mut save, 0xA0);
        save.write8(0x0E00_0010, 0x5A);
        assert_eq!(save.data[0x10], 0x5A);
        save.data[0x1000] = 0xBB;

        flash_unlock(&mut save, 0x80);
        save.write8(0x0E00_5555, 0xAA);
        save.write8(0x0E00_2AAA, 0x55);
        save.write8(0x0E00_0000, 0x30);
        assert_eq!(save.data[0x10], 0xFF);
        assert_eq!(save.data[0x1000], 0xBB);
    }

    #[test]
    fn eeprom_reads_and_writes_8k_blocks() {
        let mut save = SaveMedia::new(SaveType::Eeprom8K);
        let address = 0x123u16;
        let payload = 0x0123_4567_89AB_CDEFu64;
        write_block(&mut save, 14, address, payload);
        assert_eq!(
            &save.data[address as usize * 8..address as usize * 8 + 8],
            &payload.to_be_bytes()
        );
        assert_eq!(read_block(&mut save, 14, address), payload);
        assert!(save.dirty);
    }

    #[test]
    fn eeprom_reads_consecutive_blocks_back_to_back() {
        let mut save = SaveMedia::new(SaveType::Eeprom8K);
        let first = 0x1122_3344_5566_7788u64;
        let second = 0x99AA_BBCC_DDEE_FF00u64;
        write_block(&mut save, 14, 0, first);
        write_block(&mut save, 14, 1, second);

        // Real games issue one 68-bit read per block without an extra clock
        // pulse between requests. The second read must not return all ones.
        assert_eq!(read_block(&mut save, 14, 0), first);
        assert_eq!(read_block(&mut save, 14, 1), second);
        assert_eq!(read_block(&mut save, 14, 0), first);
    }

    #[test]
    fn eeprom_short_read_does_not_swallow_the_next_command() {
        let mut save = SaveMedia::new(SaveType::Eeprom8K);
        let first = 0x0123_4567_89AB_CDEFu64;
        write_block(&mut save, 14, 0, first);

        // Begin a read of block 0, but only clock eight reply bits.
        let mut request = vec![1, 1];
        request.extend(std::iter::repeat_n(0, 14));
        request.push(0);
        send_bits(&mut save, &request);
        for _ in 0..8 {
            let _ = save.eeprom_read_bit();
        }

        // A new write must still be accepted after the short read.
        let second = 0xFEDC_BA98_7654_3210u64;
        write_block(&mut save, 14, 1, second);
        assert_eq!(read_block(&mut save, 14, 1), second);
    }

    #[test]
    fn eeprom_dma_length_selects_512_byte_mode() {
        let mut save = SaveMedia::new(SaveType::Eeprom8K);
        save.eeprom_set_dma_length(9);
        assert_eq!(save.data.len(), 512);
        let payload = 0xDEAD_BEEF_0123_4567u64;
        write_block(&mut save, 6, 5, payload);
        assert_eq!(&save.data[40..48], &payload.to_be_bytes());
        assert_eq!(read_block(&mut save, 6, 5), payload);
    }

    #[test]
    fn bus_routes_eeprom_bits_through_the_0d_window() {
        let mut rom = vec![0u8; 0x1000];
        rom[0xA0..0xA4].copy_from_slice(b"TEST");
        rom[0xAC..0xB0].copy_from_slice(b"ABCD");
        rom[0xB0..0xB2].copy_from_slice(b"01");
        rom[0x200..0x208].copy_from_slice(b"EEPROM_V");
        let sum: u8 = rom[0xA0..=0xBC]
            .iter()
            .fold(0u8, |acc, &b| acc.wrapping_add(b));
        rom[0xBD] = 0u8.wrapping_sub(sum).wrapping_sub(0x19);

        let mut bus = Bus::new(Cartridge::new(rom));
        assert!(bus.save.is_eeprom());
        let payload = 0x1122_3344_5566_7788u64;

        // Write command: 10, 14 address bits, 64 data bits, trailing 0.
        let mut write_bits = vec![1u8, 0];
        write_bits.extend(std::iter::repeat_n(0, 14));
        for shift in (0..64).rev() {
            write_bits.push(((payload >> shift) & 1) as u8);
        }
        write_bits.push(0);
        for bit in write_bits {
            bus.write16(0x0D00_0000, u16::from(bit));
        }
        assert_eq!(&bus.save.data[0..8], &payload.to_be_bytes());

        // Read command: 11, 14 address bits, terminator. Then 4 dummy bits
        // and 64 data bits come back through the same window.
        let mut read_bits = vec![1u8, 1];
        read_bits.extend(std::iter::repeat_n(0, 14));
        read_bits.push(0);
        for bit in read_bits {
            bus.write16(0x0D00_0000, u16::from(bit));
        }
        let mut read = 0u64;
        for index in 0..68 {
            let bit = bus.read16(0x0D00_0000) & 1;
            if index >= 4 {
                read = (read << 1) | u64::from(bit);
            }
        }
        assert_eq!(read, payload);
    }
}
