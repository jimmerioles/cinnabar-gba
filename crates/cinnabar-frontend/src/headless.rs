//! The CI-friendly headless runner.

use std::collections::HashSet;
use std::fmt;
use std::path::{Path, PathBuf};

use anyhow::Result;
use cinnabar_core::{SCREEN_HEIGHT, SCREEN_WIDTH, System};

use crate::video;

/// What one headless run observed.
pub struct HeadlessReport {
    pub frames: u32,
    pub unique_colors: usize,
    pub non_black_pixels: usize,
    pub screenshot: Option<PathBuf>,
}

impl fmt::Display for HeadlessReport {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "Cinnabar headless: {} frames | {}x{} | {} unique colors | {} non-black pixels",
            self.frames, SCREEN_WIDTH, SCREEN_HEIGHT, self.unique_colors, self.non_black_pixels
        )?;
        if let Some(path) = &self.screenshot {
            write!(formatter, " | screenshot: {}", path.display())?;
        }
        Ok(())
    }
}

/// Run frames without a window or audio device and optionally save a PNG.
pub fn run(system: &mut System, frames: u32, screenshot: Option<&Path>) -> Result<HeadlessReport> {
    for _ in 0..frames {
        let _ = system.run_frame();
    }

    let framebuffer = system.framebuffer();
    let mut colors = HashSet::with_capacity(1024);
    let mut non_black_pixels = 0;
    for &pixel in framebuffer {
        let color = pixel & 0x7FFF;
        colors.insert(color);
        if color != 0 {
            non_black_pixels += 1;
        }
    }

    if let Some(path) = screenshot {
        video::save_screenshot(path, framebuffer)?;
    }

    Ok(HeadlessReport {
        frames,
        unique_colors: colors.len(),
        non_black_pixels,
        screenshot: screenshot.map(Path::to_path_buf),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use cinnabar_core::Cartridge;

    #[test]
    fn demo_renders_colors_and_writes_a_png() {
        let mut system = System::new(Cartridge::new(cinnabar_testrom::demo_rom()));
        let dir = std::env::temp_dir().join(format!(
            "cinnabar-headless-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock after epoch")
                .as_nanos()
        ));
        let path = dir.join("frame.png");

        let report = run(&mut system, 3, Some(&path)).expect("headless run");
        assert!(report.unique_colors > 1, "demo should render a gradient");
        assert!(report.non_black_pixels > 0, "demo should not be blank");
        assert_eq!(report.screenshot.as_deref(), Some(path.as_path()));
        assert_eq!(report.to_string().lines().count(), 1);

        let image = image::open(&path).expect("read screenshot").to_rgb8();
        assert_eq!(
            image.dimensions(),
            (SCREEN_WIDTH as u32, SCREEN_HEIGHT as u32)
        );

        std::fs::remove_dir_all(&dir).expect("remove temp dir");
    }

    #[test]
    fn zero_frames_is_still_a_valid_run() {
        let mut system = System::new(Cartridge::new(cinnabar_testrom::demo_rom()));
        let report = run(&mut system, 0, None).expect("headless run");
        assert_eq!(report.frames, 0);
        assert_eq!(report.screenshot, None);
    }
}
