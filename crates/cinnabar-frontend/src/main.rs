//! Cinnabar, a clean-room Game Boy Advance emulator.

mod app;
mod audio;
mod cli;
mod headless;
mod storage;
mod video;

use anyhow::Result;
use cinnabar_core::{Cartridge, System};
use clap::Parser;

use crate::cli::Args;

fn main() -> Result<()> {
    let args = Args::parse();
    args.validate()?;

    let rom_path = args.rom_path();
    let rom = match rom_path {
        Some(path) => storage::read_rom(path)?,
        None => cinnabar_testrom::logo_rom(),
    };
    let cart = Cartridge::new(rom);

    if args.info {
        cli::print_info(&cart, rom_path);
        return Ok(());
    }

    let title = cli::window_title(&cart, rom_path);
    let save_path = storage::resolve_save_path(args.save.clone(), rom_path);
    let mut system = System::new(cart);

    if let Err(error) = storage::load_save(&mut system, save_path.as_deref()) {
        eprintln!("cinnabar: warning: {error:#}");
    }

    if args.headless {
        let report = headless::run(&mut system, args.frames, args.screenshot.as_deref())?;
        println!("{report}");
        storage::persist_save(&mut system, save_path.as_deref())?;
        return Ok(());
    }

    app::run(
        system,
        app::AppConfig {
            title,
            save_path,
            scale: args.scale,
        },
    )
}
