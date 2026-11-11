//! Command-line interface and cartridge information output.

use std::path::{Path, PathBuf};

use anyhow::{Result, bail};
use cinnabar_core::Cartridge;
use clap::Parser;

/// A clean-room Game Boy Advance emulator.
#[derive(Debug, Parser)]
#[command(
    name = "cinnabar",
    version,
    about = "A clean-room Game Boy Advance emulator"
)]
pub struct Args {
    /// Path to a Game Boy Advance ROM.
    #[arg(value_name = "ROM")]
    pub rom: Option<PathBuf>,

    /// Run the bundled logo demo ROM instead of a file.
    #[arg(long, conflicts_with_all = ["rom", "builtin"])]
    pub demo: bool,

    /// Run a bundled cartridge by name (see --list-builtins).
    #[arg(long, value_name = "NAME", conflicts_with_all = ["rom", "demo"])]
    pub builtin: Option<String>,

    /// List the bundled cartridges and exit.
    #[arg(long, conflicts_with_all = ["rom", "demo", "builtin"])]
    pub list_builtins: bool,

    /// Run the built-in self-test without opening a window.
    #[arg(long, conflicts_with_all = ["rom", "demo", "builtin"])]
    pub selftest: bool,

    /// Run without a window (for CI and screenshots).
    #[arg(long)]
    pub headless: bool,

    /// Run an interactive headless debugger.
    #[arg(long, conflicts_with = "screenshot")]
    pub debug: bool,

    /// Number of frames to run in headless mode.
    #[arg(long, default_value_t = 180, value_name = "N")]
    pub frames: u32,

    /// Write a PNG of the final headless frame.
    #[arg(long, value_name = "PATH")]
    pub screenshot: Option<PathBuf>,

    /// Initial window scale factor.
    #[arg(long, value_name = "N")]
    pub scale: Option<u32>,

    /// Read settings from a TOML config file.
    #[arg(long, value_name = "PATH")]
    pub config: Option<PathBuf>,

    /// Scan a ROM library directory and list its games.
    #[arg(long, value_name = "DIR")]
    pub library: Option<PathBuf>,

    /// Play a title, game code, or file stem from the library.
    #[arg(long, value_name = "NAME", conflicts_with_all = ["rom", "demo", "builtin", "delete_save"])]
    pub play: Option<String>,

    /// Delete the save file for a library game.
    #[arg(long, value_name = "NAME", conflicts_with_all = ["rom", "demo", "builtin", "play"])]
    pub delete_save: Option<String>,

    /// Initial audio volume, from 0.0 to 1.0.
    #[arg(long, value_name = "LEVEL")]
    pub volume: Option<f32>,

    /// Post-processing effects: none, lcd, ghosting, color (comma-separated).
    #[arg(long, value_name = "EFFECTS")]
    pub shader: Option<String>,

    /// Save-file path (defaults to <rom>.sav).
    #[arg(long, value_name = "PATH")]
    pub save: Option<PathBuf>,

    /// Print cartridge metadata and exit.
    #[arg(long)]
    pub info: bool,
}

impl Args {
    /// Reject combinations that cannot produce a sensible run.
    pub fn validate(&self) -> Result<()> {
        if self.list_builtins
            || self.selftest
            || self.library.is_some()
            || self.play.is_some()
            || self.delete_save.is_some()
        {
            return Ok(());
        }
        if self.rom.is_none() && !self.demo && self.builtin.is_none() {
            bail!("no ROM given: pass a ROM path, --demo, --builtin <name>, or --selftest");
        }
        if self.scale == Some(0) {
            bail!("--scale must be at least 1");
        }
        if let Some(volume) = self.volume
            && !(0.0..=1.0).contains(&volume)
        {
            bail!("--volume must be between 0.0 and 1.0");
        }
        Ok(())
    }

    /// The ROM file path, if a bundled cartridge was not selected.
    pub fn rom_path(&self) -> Option<&Path> {
        if self.demo || self.builtin.is_some() {
            None
        } else {
            self.rom.as_deref()
        }
    }
}

/// Print the bundled cartridges and what they exercise.
pub fn print_builtins() {
    println!("Bundled cartridges:");
    println!("  logo      Red Cinnabar C on a dark blue field, with a PSG chiptune");
    println!("  tiled     Mode 0 tiles, a sprite, and alpha blending");
    println!("  input     Keypad test screen: press keys and watch boxes light up");
    println!("  irq       DMA3 fill + timer and VBlank interrupts");
    println!("  halt      VBlankIntrWait halt/wake");
    println!("  gradient  Mode 3 color test pattern");
    println!();
    println!("Run one with: cinnabar --builtin <name>");
}

/// Print the cartridge header fields the frontend understands.
pub fn print_info(cart: &Cartridge, rom_path: Option<&Path>) {
    let title = display_or_unknown(&cart.title);
    let game_code = display_or_unknown(&cart.game_code);
    let maker_code = display_or_unknown(&cart.maker_code);
    let source = rom_path
        .map(|path| path.display().to_string())
        .unwrap_or_else(|| "bundled demo ROM".to_string());

    println!("Cartridge: {source}");
    println!("  Title:       {title}");
    println!("  Game code:   {game_code}");
    println!("  Maker code:  {maker_code}");
    println!("  ROM size:    {}", human_size(cart.size()));
    println!("  Save type:   {}", cart.save_type.label());
    println!(
        "  Header:      {}",
        if cart.header_ok { "valid" } else { "invalid" }
    );
}

/// Build the window title, falling back to the file name when the header has
/// no printable title.
pub fn window_title(cart: &Cartridge, rom_path: Option<&Path>) -> String {
    let fallback = rom_path
        .and_then(Path::file_name)
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| "Demo".to_string());
    let title = if cart.title.is_empty() {
        fallback
    } else {
        cart.title.clone()
    };
    format!("Cinnabar GBA — {title}")
}

fn display_or_unknown(value: &str) -> &str {
    if value.is_empty() { "(unknown)" } else { value }
}

fn human_size(bytes: usize) -> String {
    const KIB: f64 = 1024.0;
    const MIB: f64 = 1024.0 * 1024.0;
    let bytes_f = bytes as f64;
    if bytes_f >= MIB {
        format!("{bytes} bytes ({:.1} MiB)", bytes_f / MIB)
    } else if bytes_f >= KIB {
        format!("{bytes} bytes ({:.1} KiB)", bytes_f / KIB)
    } else {
        format!("{bytes} bytes")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_a_missing_rom() {
        let args = Args::try_parse_from(["cinnabar"]).expect("valid flags");
        assert!(args.validate().is_err());
    }

    #[test]
    fn accepts_the_demo() {
        let args = Args::try_parse_from(["cinnabar", "--demo"]).expect("valid flags");
        assert!(args.validate().is_ok());
        assert_eq!(args.rom_path(), None);
    }

    #[test]
    fn accepts_the_headless_debugger() {
        let args = Args::try_parse_from(["cinnabar", "--demo", "--debug"]).expect("valid flags");
        assert!(args.validate().is_ok());
        assert!(args.debug);
    }

    #[test]
    fn accepts_a_builtin_and_selftest_without_a_rom() {
        let builtin =
            Args::try_parse_from(["cinnabar", "--builtin", "input"]).expect("valid flags");
        assert!(builtin.validate().is_ok());
        assert_eq!(builtin.rom_path(), None);

        let selftest = Args::try_parse_from(["cinnabar", "--selftest"]).expect("valid flags");
        assert!(selftest.validate().is_ok());
    }

    #[test]
    fn builtin_and_demo_conflict() {
        let parsed = Args::try_parse_from(["cinnabar", "--demo", "--builtin", "input"]);
        assert!(parsed.is_err());
    }

    #[test]
    fn rejects_zero_scale() {
        let args =
            Args::try_parse_from(["cinnabar", "--demo", "--scale", "0"]).expect("valid flags");
        assert!(args.validate().is_err());
    }

    #[test]
    fn demo_and_rom_conflict() {
        let parsed = Args::try_parse_from(["cinnabar", "game.gba", "--demo"]);
        assert!(parsed.is_err());
    }

    #[test]
    fn window_title_prefers_header_title() {
        let mut cart = Cartridge::new(vec![0; 0xC0]);
        cart.title = "POKEMON".to_string();
        assert_eq!(
            window_title(&cart, Some(Path::new("/tmp/game.gba"))),
            "Cinnabar GBA — POKEMON"
        );
    }

    #[test]
    fn window_title_falls_back_to_file_name() {
        let cart = Cartridge::new(vec![0; 0xC0]);
        assert_eq!(
            window_title(&cart, Some(Path::new("/tmp/my-game.gba"))),
            "Cinnabar GBA — my-game.gba"
        );
    }
}
