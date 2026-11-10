//! Run a ROM headlessly and write a PNG of its framebuffer.
//!
//! ```text
//! cargo run -p cinnabar-core --example screenshot -- rom.gba 180 out.png
//! ```

use std::env;
use std::error::Error;
use std::path::PathBuf;

use cinnabar_core::cart::Cartridge;
use cinnabar_core::system::System;

fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<String> = env::args().collect();
    let rom_path = args
        .get(1)
        .map(PathBuf::from)
        .ok_or("usage: screenshot <rom.gba> [frames] [out.png]")?;
    let frames = args
        .get(2)
        .and_then(|s| s.parse::<u32>().ok())
        .unwrap_or(180);
    let out_path = args
        .get(3)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("screenshot.png"));
    let keys = args
        .get(4)
        .and_then(|value| u16::from_str_radix(value, 16).ok())
        .unwrap_or(0);

    let rom = std::fs::read(&rom_path)?;
    let cart = Cartridge::new(rom);
    println!(
        "rom: {} ({}) title={:?} code={:?} save={} header_ok={}",
        rom_path.display(),
        cart.size(),
        cart.title,
        cart.game_code,
        cart.save_type.label(),
        cart.header_ok
    );

    let mut system = System::new(cart);
    system.set_keys(keys);
    let output = system.run_frames(frames);

    let mut rgb = Vec::with_capacity(240 * 160 * 3);
    let mut unique = std::collections::BTreeSet::new();
    let mut non_black = 0usize;
    for &color in &output.framebuffer {
        unique.insert(color);
        let r5 = (color & 0x1F) as u8;
        let g5 = ((color >> 5) & 0x1F) as u8;
        let b5 = ((color >> 10) & 0x1F) as u8;
        let expand = |v: u8| (v << 3) | (v >> 2);
        let (r, g, b) = (expand(r5), expand(g5), expand(b5));
        if color != 0 {
            non_black += 1;
        }
        rgb.extend_from_slice(&[r, g, b]);
    }

    image::save_buffer(&out_path, &rgb, 240, 160, image::ColorType::Rgb8)?;
    println!(
        "frames={frames} pc={:08X} r7={:08X} r12={:08X} vcount={} dispcnt={:04X} dispstat={:04X} frame_count={} line_cycles={} halted={} wait_vblank={} if={:04X} ie={:04X} ime={} audio={} unique_colors={} non_black={} -> {}",
        system.cpu.pc(),
        system.cpu.regs()[7],
        system.cpu.regs()[12],
        system.bus.ppu.vcount,
        system.bus.ppu.dispcnt,
        system.bus.ppu.dispstat,
        system.bus.ppu.frame_count,
        system.bus.ppu.line_cycles(),
        system.bus.halted,
        system.bus.wait_vblank,
        system.bus.irq.if_,
        system.bus.irq.ie,
        system.bus.irq.ime,
        output.audio.len(),
        unique.len(),
        non_black,
        out_path.display()
    );
    Ok(())
}
