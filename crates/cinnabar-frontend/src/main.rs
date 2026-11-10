//! Cinnabar, a clean-room Game Boy Advance emulator.

mod app;
mod audio;
mod cli;
mod debugger;
mod headless;
mod selftest;
mod storage;
mod video;

use anyhow::{Result, bail};
use cinnabar_core::{Cartridge, System};
use clap::Parser;

use crate::cli::Args;

fn main() -> Result<()> {
    let args = Args::parse();

    if args.list_builtins {
        cli::print_builtins();
        return Ok(());
    }
    if args.selftest {
        selftest::run()?;
        return Ok(());
    }

    args.validate()?;

    let rom_path = args.rom_path();
    let rom = match rom_path {
        Some(path) => storage::read_rom(path)?,
        None if args.demo => cinnabar_testrom::logo_rom(),
        None => builtin_rom(args.builtin.as_deref().unwrap_or("logo"))?,
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

    if args.debug {
        system = debugger::run(system, &title)?;
        storage::persist_save(&mut system, save_path.as_deref())?;
        return Ok(());
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

fn builtin_rom(name: &str) -> Result<Vec<u8>> {
    Ok(match name {
        "logo" => cinnabar_testrom::logo_rom(),
        "tiled" => cinnabar_testrom::tiled_rom(),
        "input" => cinnabar_testrom::input_rom(),
        "irq" | "dma" => cinnabar_testrom::irq_dma_rom(),
        "halt" => cinnabar_testrom::halt_rom(),
        "gradient" | "demo" => cinnabar_testrom::demo_rom(),
        other => bail!("unknown built-in cartridge '{other}'; run --list-builtins"),
    })
}
