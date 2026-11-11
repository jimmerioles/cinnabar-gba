//! ROM library scanning, cover discovery, and save-file metadata.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use cinnabar_core::Cartridge;

use crate::storage;

#[derive(Clone, Debug)]
pub struct GameEntry {
    pub path: PathBuf,
    pub title: String,
    pub game_code: String,
    pub save_type: String,
    pub save_path: PathBuf,
    pub save_size: Option<u64>,
    pub cover: Option<PathBuf>,
}

pub fn scan(directory: &Path) -> Result<Vec<GameEntry>> {
    if !directory.is_dir() {
        bail!("library directory not found: {}", directory.display());
    }

    let mut paths = Vec::new();
    collect_roms(directory, &mut paths, 0)?;
    paths.sort();

    let mut entries = Vec::new();
    for path in paths {
        let rom =
            fs::read(&path).with_context(|| format!("failed to read ROM {}", path.display()))?;
        let cart = Cartridge::new(rom);
        let title = if cart.title.is_empty() {
            path.file_stem()
                .map(|stem| stem.to_string_lossy().into_owned())
                .unwrap_or_else(|| path.display().to_string())
        } else {
            cart.title.clone()
        };
        let save_path = storage::default_save_path(&path);
        let save_size = fs::metadata(&save_path).ok().map(|metadata| metadata.len());
        let cover = find_cover(&path, &cart);
        entries.push(GameEntry {
            path,
            title,
            game_code: cart.game_code,
            save_type: cart.save_type.label().to_string(),
            save_path,
            save_size,
            cover,
        });
    }

    entries.sort_by_key(|entry| entry.title.to_lowercase());
    Ok(entries)
}

pub fn find<'a>(entries: &'a [GameEntry], query: &str) -> Option<&'a GameEntry> {
    let query = query.to_lowercase();
    entries.iter().find(|entry| {
        entry.title.to_lowercase() == query
            || entry.game_code.to_lowercase() == query
            || entry
                .path
                .file_stem()
                .map(|stem| stem.to_string_lossy().to_lowercase() == query)
                .unwrap_or(false)
    })
}

pub fn print_list(entries: &[GameEntry], directory: &Path) {
    println!(
        "Library: {} ({} game(s))",
        directory.display(),
        entries.len()
    );
    if entries.is_empty() {
        println!("  no .gba files found");
        return;
    }
    println!(
        "  {:<28} {:<5} {:<11} {:<8} COVER",
        "TITLE", "CODE", "SAVE", "SAVED"
    );
    for entry in entries {
        let saved = entry
            .save_size
            .map(|size| format!("{size}B"))
            .unwrap_or_else(|| "-".to_string());
        let cover = entry
            .cover
            .as_ref()
            .map(|path| path.display().to_string())
            .unwrap_or_else(|| "-".to_string());
        println!(
            "  {:<28} {:<5} {:<11} {:<8} {cover}",
            truncate(&entry.title, 28),
            truncate(&entry.game_code, 5),
            truncate(&entry.save_type, 11),
            saved,
        );
    }
    println!();
    println!("Play one with: cinnabar --library <dir> --play <title-or-code>");
}

fn collect_roms(directory: &Path, output: &mut Vec<PathBuf>, depth: usize) -> Result<()> {
    if depth > 2 {
        return Ok(());
    }
    for entry in fs::read_dir(directory)
        .with_context(|| format!("failed to read library directory {}", directory.display()))?
    {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            collect_roms(&path, output, depth + 1)?;
        } else if path
            .extension()
            .is_some_and(|extension| extension.eq_ignore_ascii_case("gba"))
        {
            output.push(path);
        }
    }
    Ok(())
}

fn find_cover(rom_path: &Path, cart: &Cartridge) -> Option<PathBuf> {
    let directory = rom_path.parent().unwrap_or_else(|| Path::new("."));
    let stem = rom_path.file_stem().map(|stem| stem.to_string_lossy())?;
    let mut candidates = vec![
        rom_path.with_extension("png"),
        rom_path.with_extension("jpg"),
        rom_path.with_extension("jpeg"),
    ];
    for extension in ["png", "jpg", "jpeg"] {
        candidates.push(directory.join(format!("{stem}.{extension}")));
        candidates.push(directory.join("covers").join(format!("{stem}.{extension}")));
        if !cart.game_code.is_empty() {
            candidates.push(
                directory
                    .join("covers")
                    .join(format!("{}.{}", cart.game_code, extension)),
            );
        }
    }
    candidates.into_iter().find(|candidate| candidate.is_file())
}

fn truncate(value: &str, width: usize) -> String {
    if value.chars().count() <= width {
        value.to_string()
    } else {
        value
            .chars()
            .take(width.saturating_sub(1))
            .collect::<String>()
            + "…"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_directory(name: &str) -> PathBuf {
        let directory = std::env::temp_dir().join(format!(
            "cinnabar-library-{name}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock after epoch")
                .as_nanos()
        ));
        fs::create_dir_all(&directory).expect("temp directory");
        directory
    }

    #[test]
    fn scans_roms_and_finds_cover_and_save() {
        let directory = temp_directory("scan");
        let rom_path = directory.join("demo.gba");
        fs::write(&rom_path, cinnabar_testrom::demo_rom()).expect("write rom");
        fs::write(directory.join("demo.png"), []).expect("write cover");
        fs::write(directory.join("demo.gba.sav"), [0u8; 8]).expect("write save");

        let entries = scan(&directory).expect("scan");
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].title, "CINNABAR");
        assert_eq!(
            entries[0].cover.as_deref(),
            Some(directory.join("demo.png").as_path())
        );
        assert_eq!(entries[0].save_size, Some(8));
        assert!(find(&entries, "CINNABAR").is_some());
        assert!(find(&entries, "demo").is_some());

        fs::remove_dir_all(directory).expect("remove temp directory");
    }
}
