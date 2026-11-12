//! Dump a ROM's audio to a 16-bit stereo WAV for inspection.
//!
//! ```text
//! cargo run -p cinnabar-core --example audio_dump -- rom.gba out.wav 1800
//! ```

use std::env;
use std::error::Error;
use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::PathBuf;

use cinnabar_core::cart::Cartridge;
use cinnabar_core::system::System;

fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<String> = env::args().collect();
    let rom_path = args
        .get(1)
        .map(PathBuf::from)
        .ok_or("usage: audio_dump <rom.gba> <out.wav> [frames]")?;
    let out_path = args
        .get(2)
        .map(PathBuf::from)
        .ok_or("usage: audio_dump <rom.gba> <out.wav> [frames]")?;
    let frames: u32 = args.get(3).and_then(|s| s.parse().ok()).unwrap_or(1800);

    let rom = std::fs::read(&rom_path)?;
    let mut system = System::new(Cartridge::new(rom));
    let rate = system.sample_rate();

    let mut samples = Vec::new();
    for _ in 0..frames {
        samples.extend_from_slice(&system.run_frame().audio);
    }

    let mut file = BufWriter::new(File::create(&out_path)?);
    let data_len = (samples.len() * 2) as u32;
    file.write_all(b"RIFF")?;
    file.write_all(&(36 + data_len).to_le_bytes())?;
    file.write_all(b"WAVEfmt ")?;
    file.write_all(&16u32.to_le_bytes())?;
    file.write_all(&1u16.to_le_bytes())?; // PCM
    file.write_all(&2u16.to_le_bytes())?; // stereo
    file.write_all(&rate.to_le_bytes())?;
    file.write_all(&(rate * 4).to_le_bytes())?;
    file.write_all(&4u16.to_le_bytes())?;
    file.write_all(&16u16.to_le_bytes())?;
    file.write_all(b"data")?;
    file.write_all(&data_len.to_le_bytes())?;
    for &sample in &samples {
        let value = (sample.clamp(-1.0, 1.0) * 32767.0) as i16;
        file.write_all(&value.to_le_bytes())?;
    }
    println!(
        "wrote {} frames, {} samples at {} Hz -> {}",
        frames,
        samples.len(),
        rate,
        out_path.display()
    );
    Ok(())
}
