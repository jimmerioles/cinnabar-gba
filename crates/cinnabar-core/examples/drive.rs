//! Drive a ROM headlessly through a scripted sequence of button presses.
//!
//! ```text
//! cargo run -p cinnabar-core --example drive -- rom.gba out-prefix 120:0 60:8 300:0 ...
//! ```
//!
//! Each argument is `<frames>:<keys-hex>`; a PNG is written after every step.
//! The runner also writes `<out-prefix>.state` after the last step and loads it
//! on the next run, so a scripted check can be resumed without replaying it.

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
        .ok_or("usage: drive <rom.gba> <out-prefix> <frames>:<keys-hex> ...")?;
    let prefix = args.get(2).cloned().unwrap_or_else(|| "drive".to_string());
    let steps = &args[3..];
    if steps.is_empty() {
        return Err("no steps given".into());
    }

    let rom = std::fs::read(&rom_path)?;
    let cart = Cartridge::new(rom);
    println!(
        "rom: {} title={:?} code={:?} save={}",
        rom_path.display(),
        cart.title,
        cart.game_code,
        cart.save_type.label()
    );
    let mut system = System::new(cart);
    let mut save_path = rom_path.as_os_str().to_owned();
    save_path.push(".sav");
    let save_path = PathBuf::from(save_path);
    if let Ok(bytes) = std::fs::read(&save_path) {
        system.load_save_bytes(&bytes);
        println!("loaded save {}", save_path.display());
    }
    let state_path = format!("{prefix}.state");
    if let Ok(bytes) = std::fs::read(&state_path) {
        system.load_state(&bytes)?;
        println!("loaded state {}", state_path);
    }

    let mut rgb = Vec::with_capacity(240 * 160 * 3);
    for (index, step) in steps.iter().enumerate() {
        let (frames, keys) = step
            .split_once(':')
            .ok_or_else(|| format!("bad step {step:?}, expected frames:keys"))?;
        let frames: u32 = frames.parse()?;
        let keys = u16::from_str_radix(keys, 16)?;
        system.set_keys(keys);
        for _ in 0..frames {
            let _ = system.run_frame();
        }

        rgb.clear();
        for &color in system.framebuffer() {
            let expand = |v: u16| ((v << 3) | (v >> 2)) as u8;
            rgb.extend_from_slice(&[
                expand(color & 0x1F),
                expand((color >> 5) & 0x1F),
                expand((color >> 10) & 0x1F),
            ]);
        }
        let path = format!("{prefix}-{index:02}.png");
        image::save_buffer(&path, &rgb, 240, 160, image::ColorType::Rgb8)?;
        println!(
            "step {index:02}: +{frames} frames keys={keys:03X} pc={:08X} frame={} vcount={} dispcnt={:04X} -> {path}",
            system.cpu.pc(),
            system.bus.ppu.frame_count,
            system.bus.ppu.vcount,
            system.bus.ppu.dispcnt,
        );
    }
    if system.save_dirty() {
        std::fs::write(&save_path, system.save_bytes())?;
        system.mark_save_clean();
        println!("wrote save {}", save_path.display());
    }
    std::fs::write(&state_path, system.save_state()?)?;
    println!("wrote state {}", state_path);
    Ok(())
}
