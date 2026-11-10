use cinnabar_core::{Cartridge, System};
use std::time::Instant;
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "test-roms/homebrew/celeste.gba".to_string());
    let frames: u32 = std::env::args()
        .nth(2)
        .and_then(|s| s.parse().ok())
        .unwrap_or(300);
    let keys = std::env::args()
        .nth(3)
        .and_then(|s| u16::from_str_radix(&s, 16).ok())
        .unwrap_or(0);
    let cart = Cartridge::new(std::fs::read(path)?);
    let mut system = System::new(cart);
    system.set_keys(keys);
    let start = Instant::now();
    let _ = system.run_frames(frames);
    let elapsed = start.elapsed();
    println!(
        "frames={frames} elapsed={:.3}s fps={:.1} frame_cycles={}",
        elapsed.as_secs_f64(),
        frames as f64 / elapsed.as_secs_f64(),
        system.frame_cycles()
    );
    Ok(())
}
