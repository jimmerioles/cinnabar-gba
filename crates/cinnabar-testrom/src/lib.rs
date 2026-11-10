//! Test ROMs and demo cartridges for Cinnabar.
//!
//! The binaries here are hand-written ARM assembly, assembled with `llvm-mc`
//! and checked into the repository. Rebuild them with
//! `tools/build-demo-rom.sh`.

/// A complete, legal test cartridge: header + mode-3 gradient demo.
///
/// It exercises the CPU, the cartridge bus, VRAM writes, and the PPU's bitmap
/// mode. It is the first integration test every core change must keep green.
pub const DEMO_ROM: &[u8] = include_bytes!("../roms/demo.gba");

/// A prettier mode-3 demo: a red Cinnabar "C" on a dark blue field.
pub const LOGO_ROM: &[u8] = include_bytes!("../roms/logo.gba");

/// A mode-0 demo: tiled backgrounds, a sprite, and alpha blending.
pub const TILED_ROM: &[u8] = include_bytes!("../roms/tiled.gba");

/// A DMA + timer/VBlank IRQ integration cartridge.
pub const IRQ_DMA_ROM: &[u8] = include_bytes!("../roms/irq_dma.gba");

/// A VBlankIntrWait / halt-wake cartridge.
pub const HALT_ROM: &[u8] = include_bytes!("../roms/halt.gba");

/// Owned copy, convenient for `Cartridge::new`.
pub fn demo_rom() -> Vec<u8> {
    DEMO_ROM.to_vec()
}

/// Owned copy of the logo demo.
pub fn logo_rom() -> Vec<u8> {
    LOGO_ROM.to_vec()
}

/// Owned copy of the tiled/sprite/blending demo.
pub fn tiled_rom() -> Vec<u8> {
    TILED_ROM.to_vec()
}

/// Owned copy of the DMA + IRQ cartridge.
pub fn irq_dma_rom() -> Vec<u8> {
    IRQ_DMA_ROM.to_vec()
}

/// Owned copy of the halt/wake cartridge.
pub fn halt_rom() -> Vec<u8> {
    HALT_ROM.to_vec()
}

/// A quick sanity check on the embedded ROM's header.
pub fn demo_header() -> (String, String, bool) {
    let title_end = DEMO_ROM[0xA0..0xAC]
        .iter()
        .position(|&b| b == 0)
        .unwrap_or(12);
    let title = String::from_utf8_lossy(&DEMO_ROM[0xA0..0xA0 + title_end]).to_string();
    let game_code = String::from_utf8_lossy(&DEMO_ROM[0xAC..0xB0]).to_string();
    let sum: u8 = DEMO_ROM[0xA0..=0xBC]
        .iter()
        .fold(0u8, |acc, &b| acc.wrapping_add(b));
    let expected = 0u8.wrapping_sub(sum).wrapping_sub(0x19);
    (title, game_code, DEMO_ROM[0xBD] == expected)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn header_ok(rom: &[u8]) -> bool {
        let sum: u8 = rom[0xA0..=0xBC]
            .iter()
            .fold(0u8, |acc, &b| acc.wrapping_add(b));
        let expected = 0u8.wrapping_sub(sum).wrapping_sub(0x19);
        rom[0xBD] == expected
    }

    #[test]
    fn demo_rom_has_a_valid_header() {
        assert!(DEMO_ROM.len() >= 0x8000);
        let (title, code, complement_ok) = demo_header();
        assert_eq!(title, "CINNABAR");
        assert_eq!(code, "CINB");
        assert!(complement_ok);
    }

    #[test]
    fn logo_rom_has_a_valid_header() {
        assert!(LOGO_ROM.len() >= 0x8000);
        assert_eq!(&LOGO_ROM[0xA0..0xA8], b"CINNABAR");
        assert!(header_ok(LOGO_ROM));
    }

    #[test]
    fn tiled_rom_has_a_valid_header() {
        assert!(TILED_ROM.len() >= 0x8000);
        assert_eq!(&TILED_ROM[0xA0..0xA8], b"CINNABAR");
        assert!(header_ok(TILED_ROM));
    }

    #[test]
    fn irq_dma_rom_has_a_valid_header() {
        assert!(IRQ_DMA_ROM.len() >= 0x8000);
        assert_eq!(&IRQ_DMA_ROM[0xA0..0xA8], b"CINNABAR");
        assert!(header_ok(IRQ_DMA_ROM));
    }

    #[test]
    fn halt_rom_has_a_valid_header() {
        assert!(HALT_ROM.len() >= 0x8000);
        assert_eq!(&HALT_ROM[0xA0..0xA8], b"CINNABAR");
        assert!(header_ok(HALT_ROM));
    }
}
