//! Browser arcade bindings for the headless Cinnabar core.
//!
//! This crate deliberately owns no DOM objects. JavaScript supplies the
//! canvas, audio worklet, input, and persistence; the bindings only move
//! frames, audio, and save bytes across the WebAssembly boundary. That keeps
//! the core testable and lets the page switch renderers without rebuilding
//! the emulator.

use cinnabar_core::{Cartridge, CubicResampler, SCREEN_HEIGHT, SCREEN_WIDTH, System};
use wasm_bindgen::prelude::*;

/// A cartridge that ships inside the WebAssembly bundle.
///
/// These are original test cartridges built from the assembly in this
/// repository, so the browser arcade can boot without asking for a ROM.
const BUILTINS: [&str; 6] = ["logo", "tiled", "input", "irq", "halt", "gradient"];

#[wasm_bindgen]
pub struct Emulator {
    system: System,
    resampler: Option<CubicResampler>,
    output_rate: u32,
    audio: Vec<f32>,
    rgba: Vec<u8>,
    frames: u32,
}

#[wasm_bindgen]
impl Emulator {
    /// Start a cartridge from raw ROM bytes.
    #[wasm_bindgen(constructor)]
    pub fn new(rom: &[u8]) -> Emulator {
        Self::from_rom(rom.to_vec())
    }

    /// Start one of the cartridges bundled with the page.
    pub fn builtin(name: &str) -> Result<Emulator, JsValue> {
        builtin_rom(name)
            .map(Self::from_rom)
            .ok_or_else(|| JsValue::from_str(&format!("unknown built-in cartridge: {name}")))
    }

    /// The names accepted by [`Emulator::builtin`].
    pub fn builtin_names() -> String {
        BUILTINS.join(",")
    }

    /// Run one GBA frame and queue its audio for [`Emulator::take_audio`].
    pub fn run_frame(&mut self) {
        let output = self.system.run_frame();
        self.frames = self.frames.wrapping_add(1);

        match &mut self.resampler {
            Some(resampler) => resampler.process(&output.audio, &mut self.audio),
            None => self.audio.extend_from_slice(&output.audio),
        }

        for (pixel, color) in self
            .rgba
            .chunks_exact_mut(4)
            .zip(output.framebuffer.iter().take(SCREEN_WIDTH * SCREEN_HEIGHT))
        {
            pixel[0] = expand5((color & 0x1F) as u8);
            pixel[1] = expand5(((color >> 5) & 0x1F) as u8);
            pixel[2] = expand5(((color >> 10) & 0x1F) as u8);
            pixel[3] = 255;
        }
    }

    /// A copy of the latest frame as RGBA8, ready for a canvas texture.
    pub fn framebuffer_rgba(&self) -> Vec<u8> {
        self.rgba.clone()
    }

    /// Drain the audio produced since the last call.
    pub fn take_audio(&mut self) -> Vec<f32> {
        std::mem::take(&mut self.audio)
    }

    /// Set the audio output rate. Rebuilds the resampler when it changes.
    pub fn set_output_rate(&mut self, rate: u32) {
        let rate = rate.clamp(8_000, 192_000);
        if rate == self.output_rate {
            return;
        }
        self.output_rate = rate;
        self.audio.clear();
        self.resampler = (rate != self.system.sample_rate())
            .then(|| CubicResampler::new(self.system.sample_rate(), rate));
    }

    /// The rate of the samples returned by [`Emulator::take_audio`].
    pub fn output_sample_rate(&self) -> u32 {
        self.output_rate
    }

    /// The APU's native rate, before resampling.
    pub fn source_sample_rate(&self) -> u32 {
        self.system.sample_rate()
    }

    pub fn set_keys(&mut self, keys: u16) {
        self.system.set_keys(keys);
    }

    pub fn reset(&mut self) {
        self.system.reset();
        self.frames = 0;
        self.audio.clear();
        self.resampler = (self.output_rate != self.system.sample_rate())
            .then(|| CubicResampler::new(self.system.sample_rate(), self.output_rate));
    }

    pub fn title(&self) -> String {
        if self.system.title().is_empty() {
            "Untitled cartridge".to_string()
        } else {
            self.system.title().to_string()
        }
    }

    pub fn game_code(&self) -> String {
        self.system.game_code().to_string()
    }

    /// The detected backup type, for display in the page.
    pub fn save_type(&self) -> String {
        self.system.save_type().label().to_string()
    }

    /// Battery-backed save bytes. Empty for cartridges without a save.
    pub fn save_data(&self) -> Vec<u8> {
        self.system.save_bytes().to_vec()
    }

    /// Load `.sav` bytes into the battery-backed save media.
    pub fn load_save(&mut self, bytes: &[u8]) {
        self.system.load_save_bytes(bytes);
    }

    pub fn save_dirty(&self) -> bool {
        self.system.save_dirty()
    }

    pub fn mark_save_clean(&mut self) {
        self.system.mark_save_clean();
    }

    pub fn mark_save_dirty(&mut self) {
        self.system.mark_save_dirty();
    }

    pub fn frame_count(&self) -> u32 {
        self.frames
    }

    pub fn rom_size(&self) -> u32 {
        self.system.bus.cart.size() as u32
    }

    fn from_rom(rom: Vec<u8>) -> Self {
        let system = System::new(Cartridge::new(rom));
        let output_rate = system.sample_rate();
        Self {
            system,
            resampler: None,
            output_rate,
            audio: Vec::new(),
            rgba: vec![0; SCREEN_WIDTH * SCREEN_HEIGHT * 4],
            frames: 0,
        }
    }
}

fn builtin_rom(name: &str) -> Option<Vec<u8>> {
    match name {
        "logo" => Some(cinnabar_testrom::logo_rom()),
        "tiled" => Some(cinnabar_testrom::tiled_rom()),
        "input" => Some(cinnabar_testrom::input_rom()),
        "irq" => Some(cinnabar_testrom::irq_dma_rom()),
        "halt" => Some(cinnabar_testrom::halt_rom()),
        "gradient" | "demo" => Some(cinnabar_testrom::demo_rom()),
        _ => None,
    }
}

const fn expand5(value: u8) -> u8 {
    (value << 3) | (value >> 2)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sram_demo_rom() -> Vec<u8> {
        let mut rom = cinnabar_testrom::demo_rom();
        rom[0x200..0x206].copy_from_slice(b"SRAM_V");
        rom
    }

    #[test]
    fn builtin_names_cover_every_cartridge() {
        assert_eq!(
            Emulator::builtin_names(),
            "logo,tiled,input,irq,halt,gradient"
        );
        for name in BUILTINS {
            assert!(builtin_rom(name).is_some(), "missing builtin {name}");
        }
        assert!(builtin_rom("demo").is_some());
        assert!(builtin_rom("missing").is_none());
        assert!(Emulator::builtin("logo").is_ok());
    }

    #[test]
    fn rgba_framebuffer_matches_the_core() {
        let rom = cinnabar_testrom::demo_rom();
        let mut emulator = Emulator::new(&rom);
        for _ in 0..3 {
            emulator.run_frame();
        }

        let mut system = System::new(Cartridge::new(rom));
        let _ = system.run_frames(3);
        assert_eq!(emulator.frame_count(), 3);

        let rgba = emulator.framebuffer_rgba();
        assert_eq!(rgba.len(), SCREEN_WIDTH * SCREEN_HEIGHT * 4);
        for (index, &color) in system.framebuffer().iter().enumerate() {
            let pixel = &rgba[index * 4..index * 4 + 4];
            assert_eq!(pixel[0], expand5((color & 0x1F) as u8));
            assert_eq!(pixel[1], expand5(((color >> 5) & 0x1F) as u8));
            assert_eq!(pixel[2], expand5(((color >> 10) & 0x1F) as u8));
            assert_eq!(pixel[3], 255);
        }
    }

    #[test]
    fn audio_drains_and_resamples() {
        let mut emulator = Emulator::new(&cinnabar_testrom::logo_rom());
        emulator.run_frame();
        let native = emulator.take_audio();
        assert!(!native.is_empty());
        assert_eq!(native.len() % 2, 0);
        assert!(emulator.take_audio().is_empty());
        assert_eq!(emulator.output_sample_rate(), 48_000);
        assert_eq!(emulator.source_sample_rate(), 48_000);

        let mut resampled = Emulator::new(&cinnabar_testrom::logo_rom());
        resampled.set_output_rate(24_000);
        for _ in 0..4 {
            resampled.run_frame();
        }
        let audio = resampled.take_audio();
        assert!(!audio.is_empty());
        assert_eq!(resampled.output_sample_rate(), 24_000);

        let mut native = Emulator::new(&cinnabar_testrom::logo_rom());
        for _ in 0..4 {
            native.run_frame();
        }
        let native_audio = native.take_audio();
        let ratio = audio.len() as f64 / native_audio.len() as f64;
        assert!((0.45..=0.55).contains(&ratio), "rate ratio {ratio}");
    }

    #[test]
    fn output_rate_is_clamped() {
        let mut emulator = Emulator::new(&cinnabar_testrom::demo_rom());
        emulator.set_output_rate(1);
        assert_eq!(emulator.output_sample_rate(), 8_000);
        emulator.set_output_rate(u32::MAX);
        assert_eq!(emulator.output_sample_rate(), 192_000);
    }

    #[test]
    fn save_bytes_round_trip_through_the_bindings() {
        let mut emulator = Emulator::new(&sram_demo_rom());
        assert_eq!(emulator.save_type(), "SRAM 32K");
        assert_eq!(emulator.save_data().len(), 32 * 1024);
        assert!(!emulator.save_dirty());

        let mut save = vec![0xFFu8; 32 * 1024];
        save[0] = 0xAB;
        save[0x42] = 0xCD;
        emulator.load_save(&save);
        assert_eq!(emulator.save_data(), save);
        assert!(!emulator.save_dirty());

        emulator.mark_save_dirty();
        assert!(emulator.save_dirty());
        emulator.mark_save_clean();
        assert!(!emulator.save_dirty());
    }

    #[test]
    fn metadata_and_reset_are_exposed() {
        let rom = cinnabar_testrom::demo_rom();
        let mut emulator = Emulator::new(&rom);
        assert_eq!(emulator.title(), "CINNABAR");
        assert_eq!(emulator.game_code(), "CINB");
        assert_eq!(emulator.rom_size() as usize, rom.len());
        assert_eq!(emulator.frame_count(), 0);

        emulator.run_frame();
        assert_eq!(emulator.frame_count(), 1);
        emulator.set_keys(0x0001);
        emulator.reset();
        assert_eq!(emulator.frame_count(), 0);
        assert!(emulator.take_audio().is_empty());

        let blank = Emulator::new(&[0u8; 0x100]);
        assert_eq!(blank.title(), "Untitled cartridge");
    }
}
