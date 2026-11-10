//! ROM loading and battery-backed save persistence.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use cinnabar_core::System;

/// Read a ROM from disk.
pub fn read_rom(path: &Path) -> Result<Vec<u8>> {
    fs::read(path).with_context(|| format!("failed to read ROM {}", path.display()))
}

/// The default battery save path for a cartridge: `<rom>.sav`.
pub fn default_save_path(rom_path: &Path) -> PathBuf {
    let mut path = rom_path.as_os_str().to_owned();
    path.push(".sav");
    PathBuf::from(path)
}

/// Resolve the save path from the explicit CLI flag or the ROM location.
pub fn resolve_save_path(explicit: Option<PathBuf>, rom_path: Option<&Path>) -> Option<PathBuf> {
    explicit.or_else(|| rom_path.map(default_save_path))
}

/// Load save media from disk if the file exists.
///
/// A missing file is expected on a first run and is not an error. The caller
/// decides whether other I/O errors should be fatal.
pub fn load_save(system: &mut System, path: Option<&Path>) -> Result<()> {
    let Some(path) = path else {
        return Ok(());
    };
    if system.bus.save.is_empty() || !path.exists() {
        return Ok(());
    }

    let bytes =
        fs::read(path).with_context(|| format!("failed to read save {}", path.display()))?;
    system.bus.save.load(&bytes);
    Ok(())
}

/// Write dirty save media to disk. Clean media and cartridges without a
/// battery are ignored.
pub fn persist_save(system: &mut System, path: Option<&Path>) -> Result<()> {
    if !system.bus.save.dirty || system.bus.save.is_empty() {
        return Ok(());
    }

    let Some(path) = path else {
        eprintln!("cinnabar: warning: save data is dirty but no save path is configured");
        return Ok(());
    };

    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create save directory {}", parent.display()))?;
    }
    fs::write(path, system.bus.save.bytes())
        .with_context(|| format!("failed to write save {}", path.display()))?;
    system.bus.save.mark_clean();
    eprintln!("cinnabar: wrote save {}", path.display());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use cinnabar_core::{Cartridge, SaveType};

    #[test]
    fn default_save_path_appends_sav() {
        assert_eq!(
            default_save_path(Path::new("/tmp/game.gba")),
            PathBuf::from("/tmp/game.gba.sav")
        );
    }

    #[test]
    fn explicit_save_path_wins() {
        assert_eq!(
            resolve_save_path(
                Some(PathBuf::from("/tmp/custom.sav")),
                Some(Path::new("r.gba"))
            ),
            Some(PathBuf::from("/tmp/custom.sav"))
        );
    }

    #[test]
    fn no_rom_means_no_default_save_path() {
        assert_eq!(resolve_save_path(None, None), None);
    }

    #[test]
    fn round_trips_save_media() {
        let dir = std::env::temp_dir().join(format!(
            "cinnabar-save-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock after epoch")
                .as_nanos()
        ));
        fs::create_dir_all(&dir).expect("temp dir");
        let path = dir.join("game.sav");

        let mut rom = vec![0u8; 0x1000];
        rom[0x200..0x206].copy_from_slice(b"SRAM_V");
        let mut system = System::new(Cartridge::new(rom));
        assert_eq!(system.bus.save.kind, SaveType::Sram);

        system.bus.save.data[0x42] = 0xAB;
        system.bus.save.dirty = true;
        persist_save(&mut system, Some(&path)).expect("save");
        assert!(!system.bus.save.dirty);
        assert!(path.exists());

        let mut restored = System::new(Cartridge::new(vec![0; 0x1000]));
        restored.bus.save.kind = SaveType::Sram;
        restored.bus.save.data = vec![0xFF; SaveType::Sram.size()];
        load_save(&mut restored, Some(&path)).expect("load");
        assert_eq!(restored.bus.save.data[0x42], 0xAB);

        fs::remove_dir_all(&dir).expect("remove temp dir");
    }
}
