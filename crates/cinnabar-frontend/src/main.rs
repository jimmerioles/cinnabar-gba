//! Cinnabar, a clean-room Game Boy Advance emulator.

mod app;
mod audio;
mod cli;
mod config;
mod debugger;
mod gamepad;
mod headless;
mod keymap;
mod library;
mod selftest;
mod storage;
mod video;

use std::fs;
use std::path::Path;

use anyhow::{Context, Result, bail};
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
    let file_config = config::load(args.config.as_deref())?;

    let scale = args.scale.or(file_config.scale).unwrap_or(4);
    if scale == 0 {
        bail!("--scale must be at least 1");
    }
    let volume = args.volume.or(file_config.volume).unwrap_or(1.0);
    if !(0.0..=1.0).contains(&volume) {
        bail!("volume must be between 0.0 and 1.0");
    }
    let shader = args
        .shader
        .as_deref()
        .or(file_config.shader.as_deref())
        .map(video::Shader::parse)
        .transpose()?
        .unwrap_or_default();
    let layout = args
        .keyboard_layout
        .or(file_config.keyboard_layout)
        .unwrap_or_default();
    let keymap = keymap::KeyMap::from_config(layout, &file_config.keys)?;

    let library_dir = args.library.clone().or(file_config.library.clone());
    let mut selected_rom = args.rom_path().map(Path::to_path_buf);
    if let Some(directory) = library_dir {
        let entries = library::scan(&directory)?;
        if let Some(query) = &args.delete_save {
            let entry = library::find(&entries, query)
                .with_context(|| format!("no library game matched \"{query}\""))?;
            if entry.save_path.exists() {
                fs::remove_file(&entry.save_path)
                    .with_context(|| format!("failed to remove {}", entry.save_path.display()))?;
                println!("deleted save {}", entry.save_path.display());
            } else {
                println!("no save file for {}", entry.title);
            }
            return Ok(());
        }
        if let Some(query) = &args.play {
            let entry = library::find(&entries, query)
                .with_context(|| format!("no library game matched \"{query}\""))?;
            selected_rom = Some(entry.path.clone());
            println!(
                "cinnabar: playing {} from {}",
                entry.title,
                entry.path.display()
            );
        } else {
            library::print_list(&entries, &directory);
            return Ok(());
        }
    } else if args.play.is_some() || args.delete_save.is_some() {
        bail!("--play/--delete-save require --library or a library path in the config");
    }

    let rom_path = selected_rom.as_deref();
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
            scale,
            shader,
            volume,
            keymap,
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
