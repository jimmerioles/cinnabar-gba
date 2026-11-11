//! Framebuffer conversion, PNG export, and the softbuffer window surface.

use std::fs;
use std::num::NonZeroU32;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};
use cinnabar_core::{SCREEN_HEIGHT, SCREEN_WIDTH};
use image::{ImageFormat, RgbImage};
use winit::window::Window;

/// Convert one BGR555 pixel to 8-bit RGB with 5-to-8-bit expansion.
pub fn bgr555_to_rgb(color: u16) -> [u8; 3] {
    let r = (color & 0x1F) as u8;
    let g = ((color >> 5) & 0x1F) as u8;
    let b = ((color >> 10) & 0x1F) as u8;
    [expand5(r), expand5(g), expand5(b)]
}

const fn expand5(value: u8) -> u8 {
    (value << 3) | (value >> 2)
}

/// Optional post-processing effects applied while presenting.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Shader {
    pub lcd: bool,
    pub ghosting: bool,
    pub color: bool,
}

impl Shader {
    pub fn parse(value: &str) -> Result<Self> {
        let mut shader = Self::default();
        for part in value.split([',', '+', ' ']).filter(|part| !part.is_empty()) {
            match part.to_ascii_lowercase().as_str() {
                "none" | "off" => shader = Self::default(),
                "lcd" | "grid" => shader.lcd = true,
                "ghosting" | "ghost" => shader.ghosting = true,
                "color" | "colour" => shader.color = true,
                other => anyhow::bail!("unknown shader \"{other}\""),
            }
        }
        Ok(shader)
    }

    pub fn label(self) -> String {
        let mut parts = Vec::new();
        if self.lcd {
            parts.push("lcd");
        }
        if self.ghosting {
            parts.push("ghosting");
        }
        if self.color {
            parts.push("color");
        }
        if parts.is_empty() {
            "none".to_string()
        } else {
            parts.join("+")
        }
    }
}

fn color_correct([red, green, blue]: [u8; 3]) -> [u8; 3] {
    // A soft GBA-LCD approximation: slightly desaturated and warmed, with
    // lifted blacks and reduced contrast.
    let r = 0.82 * f32::from(red) + 0.14 * f32::from(green) + 0.04 * f32::from(blue) + 4.0;
    let g = 0.08 * f32::from(red) + 0.88 * f32::from(green) + 0.04 * f32::from(blue) + 3.0;
    let b = 0.05 * f32::from(red) + 0.09 * f32::from(green) + 0.86 * f32::from(blue) + 2.0;
    [clamp8(r), clamp8(g), clamp8(b)]
}

fn clamp8(value: f32) -> u8 {
    value.clamp(0.0, 255.0) as u8
}

fn blend(current: [u8; 3], previous: [u8; 3], previous_weight: f32) -> [u8; 3] {
    let current_weight = 1.0 - previous_weight;
    [
        clamp8(f32::from(current[0]) * current_weight + f32::from(previous[0]) * previous_weight),
        clamp8(f32::from(current[1]) * current_weight + f32::from(previous[1]) * previous_weight),
        clamp8(f32::from(current[2]) * current_weight + f32::from(previous[2]) * previous_weight),
    ]
}

fn darken(pixel: u32, factor: f32) -> u32 {
    let r = ((pixel >> 16) & 0xFF) as f32 * factor;
    let g = ((pixel >> 8) & 0xFF) as f32 * factor;
    let b = (pixel & 0xFF) as f32 * factor;
    ((clamp8(r) as u32) << 16) | ((clamp8(g) as u32) << 8) | clamp8(b) as u32
}

fn rgb_to_pixel([r, g, b]: [u8; 3]) -> u32 {
    ((r as u32) << 16) | ((g as u32) << 8) | b as u32
}

/// Convert a full 240x160 BGR555 framebuffer to packed RGB8.
pub fn framebuffer_to_rgb8(framebuffer: &[u16]) -> Vec<u8> {
    let mut rgb = Vec::with_capacity(SCREEN_WIDTH * SCREEN_HEIGHT * 3);
    for y in 0..SCREEN_HEIGHT {
        for x in 0..SCREEN_WIDTH {
            let color = framebuffer.get(y * SCREEN_WIDTH + x).copied().unwrap_or(0);
            rgb.extend_from_slice(&bgr555_to_rgb(color));
        }
    }
    rgb
}

/// Write a framebuffer as a PNG, creating parent directories as needed.
pub fn save_screenshot(path: &Path, framebuffer: &[u16]) -> Result<()> {
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent).with_context(|| {
            format!("failed to create screenshot directory {}", parent.display())
        })?;
    }

    let rgb = framebuffer_to_rgb8(framebuffer);
    let image = RgbImage::from_raw(SCREEN_WIDTH as u32, SCREEN_HEIGHT as u32, rgb)
        .context("framebuffer has the wrong size for a 240x160 image")?;
    image
        .save_with_format(path, ImageFormat::Png)
        .with_context(|| format!("failed to write screenshot {}", path.display()))?;
    Ok(())
}

/// A timestamped path under `screenshots/` for an interactive screenshot.
pub fn screenshot_path() -> PathBuf {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_millis())
        .unwrap_or(0);
    PathBuf::from("screenshots").join(format!("cinnabar-{millis}.png"))
}

/// Largest integer scale that fits the emulated screen in a window.
pub fn fit_scale(width: u32, height: u32) -> u32 {
    (width / SCREEN_WIDTH as u32)
        .min(height / SCREEN_HEIGHT as u32)
        .max(1)
}

/// A softbuffer-backed window surface with integer nearest-neighbor scaling.
pub struct WindowSurface {
    _context: softbuffer::Context<Arc<Window>>,
    surface: softbuffer::Surface<Arc<Window>, Arc<Window>>,
    width: u32,
    height: u32,
    scaled_row: Vec<u32>,
    scaled_row_dim: Vec<u32>,
    current: Vec<[u8; 3]>,
    previous: Vec<[u8; 3]>,
    first_frame: bool,
    shader: Shader,
}

impl WindowSurface {
    pub fn new(window: Arc<Window>) -> Result<Self> {
        let context = softbuffer_result(
            softbuffer::Context::new(window.clone()),
            "failed to create the software rendering context",
        )?;
        let mut surface = softbuffer_result(
            softbuffer::Surface::new(&context, window.clone()),
            "failed to create the window drawing surface",
        )?;

        let size = window.inner_size();
        let width = size.width.max(1);
        let height = size.height.max(1);
        softbuffer_result(
            surface.resize(nonzero(width), nonzero(height)),
            "failed to size the window drawing surface",
        )?;

        Ok(Self {
            _context: context,
            surface,
            width,
            height,
            scaled_row: Vec::new(),
            scaled_row_dim: Vec::new(),
            current: vec![[0, 0, 0]; SCREEN_WIDTH * SCREEN_HEIGHT],
            previous: vec![[0, 0, 0]; SCREEN_WIDTH * SCREEN_HEIGHT],
            first_frame: true,
            shader: Shader::default(),
        })
    }

    pub fn set_shader(&mut self, shader: Shader) {
        self.shader = shader;
    }

    pub fn resize(&mut self, width: u32, height: u32) -> Result<()> {
        if width == 0 || height == 0 {
            return Ok(());
        }
        softbuffer_result(
            self.surface.resize(nonzero(width), nonzero(height)),
            "failed to resize the window drawing surface",
        )?;
        self.width = width;
        self.height = height;
        Ok(())
    }

    /// Present the framebuffer, centered on a black letterboxed background.
    pub fn present(&mut self, framebuffer: &[u16]) -> Result<()> {
        if self.width == 0 || self.height == 0 {
            return Ok(());
        }

        let mut buffer = softbuffer_result(
            self.surface.buffer_mut(),
            "failed to acquire a software frame buffer",
        )?;
        if buffer.len() != self.width as usize * self.height as usize {
            anyhow::bail!(
                "software frame buffer is {} pixels, expected {}",
                buffer.len(),
                self.width as usize * self.height as usize
            );
        }
        buffer.fill(0);

        let width = self.width as usize;
        let height = self.height as usize;
        let scale = fit_scale(self.width, self.height) as usize;
        let scaled_width = (SCREEN_WIDTH * scale).min(width);
        let scaled_height = (SCREEN_HEIGHT * scale).min(height);
        let left = (width - scaled_width) / 2;
        let top = (height - scaled_height) / 2;

        // Convert and post-process at native resolution first, then scale.
        for (index, color) in framebuffer
            .iter()
            .take(SCREEN_WIDTH * SCREEN_HEIGHT)
            .enumerate()
        {
            let mut rgb = bgr555_to_rgb(*color);
            if self.shader.color {
                rgb = color_correct(rgb);
            }
            if self.shader.ghosting && !self.first_frame {
                rgb = blend(rgb, self.previous[index], 0.25);
            }
            self.current[index] = rgb;
            self.previous[index] = rgb;
        }
        self.first_frame = false;

        self.scaled_row.resize(scaled_width, 0);
        self.scaled_row_dim.resize(scaled_width, 0);
        for source_y in 0..SCREEN_HEIGHT {
            let dest_y = source_y * scale;
            if dest_y >= scaled_height {
                break;
            }

            for (dest_x, pixel) in self.scaled_row.iter_mut().enumerate() {
                let source_x = dest_x / scale;
                let rgb = self.current[source_y * SCREEN_WIDTH + source_x];
                let mut packed = rgb_to_pixel(rgb);
                if self.shader.lcd && dest_x % scale == 0 {
                    packed = darken(packed, 0.72);
                }
                *pixel = packed;
            }
            if self.shader.lcd {
                for (dest, pixel) in self.scaled_row_dim.iter_mut().zip(self.scaled_row.iter()) {
                    *dest = darken(*pixel, 0.72);
                }
            }

            let rows = scale.min(scaled_height - dest_y);
            for row in 0..rows {
                let dest_row = dest_y + row;
                let source = if self.shader.lcd && dest_row % scale == 0 {
                    &self.scaled_row_dim
                } else {
                    &self.scaled_row
                };
                let start = (top + dest_row) * width + left;
                buffer[start..start + scaled_width].copy_from_slice(&source[..scaled_width]);
            }
        }

        softbuffer_result(buffer.present(), "failed to present the frame")?;
        Ok(())
    }
}

fn softbuffer_result<T>(
    result: Result<T, softbuffer::SoftBufferError>,
    message: &str,
) -> Result<T> {
    result.map_err(|error| anyhow::anyhow!("{message}: {error}"))
}

fn nonzero(value: u32) -> NonZeroU32 {
    NonZeroU32::new(value.max(1)).expect("value is at least one")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expands_five_bit_channels_to_full_range() {
        assert_eq!(bgr555_to_rgb(0x0000), [0, 0, 0]);
        assert_eq!(bgr555_to_rgb(0x001F), [255, 0, 0]);
        assert_eq!(bgr555_to_rgb(0x03E0), [0, 255, 0]);
        assert_eq!(bgr555_to_rgb(0x7C00), [0, 0, 255]);
        assert_eq!(bgr555_to_rgb(0x7FFF), [255, 255, 255]);
    }

    #[test]
    fn packs_rgb_into_softbuffer_layout() {
        assert_eq!(rgb_to_pixel([0x12, 0x34, 0x56]), 0x0012_3456);
    }

    #[test]
    fn parses_shader_combinations() {
        assert_eq!(Shader::parse("none").unwrap(), Shader::default());
        assert_eq!(
            Shader::parse("lcd,color").unwrap(),
            Shader {
                lcd: true,
                ghosting: false,
                color: true,
            }
        );
        assert!(Shader::parse("bloom").is_err());
    }

    #[test]
    fn color_correction_stays_in_range_and_changes_output() {
        let corrected = color_correct([255, 0, 0]);
        assert!(corrected[0] > 180);
        assert_ne!(color_correct([10, 20, 30]), [10, 20, 30]);
    }

    #[test]
    fn fits_integer_scales_to_the_window() {
        assert_eq!(fit_scale(960, 640), 4);
        assert_eq!(fit_scale(1000, 700), 4);
        assert_eq!(fit_scale(800, 600), 3);
        assert_eq!(fit_scale(239, 159), 1);
    }

    #[test]
    fn writes_a_valid_png() {
        let dir = std::env::temp_dir().join(format!(
            "cinnabar-png-test-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("clock after epoch")
                .as_nanos()
        ));
        let path = dir.join("nested/screenshot.png");
        let mut framebuffer = vec![0u16; SCREEN_WIDTH * SCREEN_HEIGHT];
        framebuffer[0] = 0x001F;
        framebuffer[1] = 0x7FFF;

        save_screenshot(&path, &framebuffer).expect("write screenshot");
        let image = image::open(&path).expect("read screenshot").to_rgb8();
        assert_eq!(image.dimensions(), (240, 160));
        assert_eq!(image.get_pixel(0, 0).0, [255, 0, 0]);
        assert_eq!(image.get_pixel(1, 0).0, [255, 255, 255]);
        assert_eq!(image.get_pixel(2, 0).0, [0, 0, 0]);

        fs::remove_dir_all(&dir).expect("remove temp dir");
    }
}
