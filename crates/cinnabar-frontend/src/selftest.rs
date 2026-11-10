//! A no-window, no-audio self-test for the assembled emulator.
//!
//! This is the easiest way for a human to check that the core, the bundled
//! test cartridges, the PPU, the keypad and the interrupt path are all alive
//! without finding a commercial ROM or configuring a window.

use std::collections::BTreeSet;
use std::fmt::Display;

use anyhow::{Result, bail};
use cinnabar_core::keypad::keys;
use cinnabar_core::{Cartridge, System};
use cinnabar_testrom::{demo_rom, halt_rom, input_rom, irq_dma_rom, logo_rom, tiled_rom};

pub fn run() -> Result<()> {
    println!("Cinnabar GBA self-test");
    let mut failures = 0u32;

    let (_, fb) = run_rom(logo_rom(), 3);
    let colors = unique_colors(&fb);
    report(
        &mut failures,
        "logo demo",
        colors >= 3 && fb[0] == 0x0C00 && pixel(&fb, 120, 40) == 0x001F,
        format!("{colors} colors, red C on dark blue"),
    );

    let (_, fb) = run_rom(tiled_rom(), 3);
    let colors = unique_colors(&fb);
    let sprite = pixel(&fb, 104, 64);
    report(
        &mut failures,
        "tiled PPU",
        colors >= 5 && sprite != 0 && sprite != 0x001F,
        format!("{colors} colors, blended sprite"),
    );

    let (mut system, _) = run_rom(input_rom(), 3);
    let off = pixel(system.framebuffer(), 188, 56);
    system.set_keys(keys::A);
    let _ = system.run_frames(2);
    let a_on = pixel(system.framebuffer(), 188, 56);
    system.set_keys(keys::RIGHT);
    let _ = system.run_frames(2);
    let right_on = pixel(system.framebuffer(), 84, 76);
    system.set_keys(keys::START);
    let _ = system.run_frames(2);
    let start_on = pixel(system.framebuffer(), 100, 137);
    report(
        &mut failures,
        "keypad",
        off == 0x1CE7 && a_on == 0x001F && right_on == 0x03E0 && start_on == 0x7FFF,
        "A, right and Start indicators",
    );

    let (system, fb) = run_rom(irq_dma_rom(), 8);
    let vblank = u32::from_le_bytes(system.bus.iwram[0..4].try_into().unwrap());
    let timer = u32::from_le_bytes(system.bus.iwram[4..8].try_into().unwrap());
    report(
        &mut failures,
        "DMA + IRQ",
        fb[0] == 0x03E0 && fb[1] == 0x0C00 && vblank > 0 && timer > 0 && system.bus.irq.if_ == 0,
        format!("{vblank} VBlank, {timer} timer IRQs"),
    );

    let (system, fb) = run_rom(halt_rom(), 8);
    let frames = u32::from_le_bytes(system.bus.iwram[0..4].try_into().unwrap());
    report(
        &mut failures,
        "VBlankIntrWait",
        fb[0] == 0x7FFF && frames > 0 && system.bus.halted,
        format!("{frames} wake-ups"),
    );

    let (_, fb) = run_rom(demo_rom(), 3);
    let colors = unique_colors(&fb);
    report(
        &mut failures,
        "mode 3 gradient",
        colors > 1000,
        format!("{colors} colors"),
    );

    if failures > 0 {
        bail!("{failures} self-test check(s) failed");
    }
    println!("all 6 checks passed");
    Ok(())
}

fn run_rom(rom: Vec<u8>, frames: u32) -> (System, Vec<u16>) {
    let mut system = System::new(Cartridge::new(rom));
    let output = system.run_frames(frames);
    (system, output.framebuffer)
}

fn pixel(framebuffer: &[u16], x: usize, y: usize) -> u16 {
    framebuffer[y * 240 + x]
}

fn unique_colors(framebuffer: &[u16]) -> usize {
    framebuffer.iter().copied().collect::<BTreeSet<_>>().len()
}

fn report(failures: &mut u32, name: &str, ok: bool, detail: impl Display) {
    if ok {
        println!("  [ok]   {name}: {detail}");
    } else {
        println!("  [FAIL] {name}: {detail}");
        *failures += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_checks_pass() {
        assert!(run().is_ok());
    }
}
