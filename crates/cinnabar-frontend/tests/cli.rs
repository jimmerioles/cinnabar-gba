//! End-to-end checks of the `cinnabar` binary's CI paths.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

fn unique_dir(name: &str) -> PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock after epoch")
        .as_nanos();
    std::env::temp_dir().join(format!(
        "cinnabar-cli-{name}-{}-{stamp}",
        std::process::id()
    ))
}

fn temp_dir(name: &str) -> PathBuf {
    let dir = unique_dir(name);
    fs::create_dir_all(&dir).expect("create temp dir");
    dir
}

fn write_demo_rom(directory: &Path) -> PathBuf {
    let path = directory.join("demo.gba");
    fs::write(&path, cinnabar_testrom::demo_rom()).expect("write demo rom");
    path
}

fn run_debugger(rom: &Path, save: &Path, commands: &str) -> Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_cinnabar"))
        .arg(rom)
        .args(["--debug", "--save"])
        .arg(save)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn cinnabar");
    child
        .stdin
        .as_mut()
        .expect("debugger stdin")
        .write_all(commands.as_bytes())
        .expect("write debugger commands");
    child.wait_with_output().expect("wait for cinnabar")
}

#[test]
fn demo_info_prints_the_header() {
    let output = Command::new(env!("CARGO_BIN_EXE_cinnabar"))
        .args(["--demo", "--info"])
        .output()
        .expect("run cinnabar");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Title:       CINNABAR"), "{stdout}");
    assert!(stdout.contains("Game code:   CINB"), "{stdout}");
    assert!(stdout.contains("Save type:   none"), "{stdout}");
    assert!(stdout.contains("Header:      valid"), "{stdout}");
}

#[test]
fn headless_demo_writes_a_png_and_a_summary() {
    let dir = unique_dir("headless");
    let path = dir.join("nested/demo.png");
    let output = Command::new(env!("CARGO_BIN_EXE_cinnabar"))
        .args(["--demo", "--headless", "--frames", "3", "--screenshot"])
        .arg(&path)
        .output()
        .expect("run cinnabar");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(stdout.lines().count(), 1, "{stdout}");
    assert!(stdout.contains("3 frames"), "{stdout}");
    assert!(stdout.contains("240x160"), "{stdout}");
    assert!(stdout.contains("unique colors"), "{stdout}");
    assert!(stdout.contains("non-black pixels"), "{stdout}");

    let image = image::open(&path).expect("read screenshot").to_rgb8();
    assert_eq!(image.dimensions(), (240, 160));

    fs::remove_dir_all(&dir).expect("remove temp dir");
}

#[test]
fn library_lists_roms_and_covers() {
    let dir = temp_dir("library-list");
    write_demo_rom(&dir);
    fs::write(dir.join("demo.png"), []).expect("write cover");

    let output = Command::new(env!("CARGO_BIN_EXE_cinnabar"))
        .args(["--library"])
        .arg(&dir)
        .output()
        .expect("run cinnabar");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("(1 game(s))"), "{stdout}");
    assert!(stdout.contains("CINNABAR"), "{stdout}");
    assert!(stdout.contains("CINB"), "{stdout}");
    assert!(stdout.contains("demo.png"), "{stdout}");

    fs::remove_dir_all(&dir).expect("remove temp dir");
}

#[test]
fn library_play_boots_the_selected_rom() {
    let dir = temp_dir("library-play");
    write_demo_rom(&dir);

    let output = Command::new(env!("CARGO_BIN_EXE_cinnabar"))
        .args(["--library"])
        .arg(&dir)
        .args(["--play", "CINB", "--headless", "--frames", "3"])
        .output()
        .expect("run cinnabar");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("playing CINNABAR"), "{stdout}");
    assert!(stdout.contains("Cinnabar headless: 3 frames"), "{stdout}");

    fs::remove_dir_all(&dir).expect("remove temp dir");
}

#[test]
fn library_reports_an_unknown_game() {
    let dir = temp_dir("library-missing");
    write_demo_rom(&dir);

    let output = Command::new(env!("CARGO_BIN_EXE_cinnabar"))
        .args(["--library"])
        .arg(&dir)
        .args(["--play", "MISSING"])
        .output()
        .expect("run cinnabar");
    assert!(!output.status.success());

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("no library game matched"), "{stderr}");

    fs::remove_dir_all(&dir).expect("remove temp dir");
}

#[test]
fn missing_rom_fails_with_a_clear_error() {
    let path = unique_dir("missing-rom").join("nope.gba");
    let output = Command::new(env!("CARGO_BIN_EXE_cinnabar"))
        .arg(&path)
        .arg("--info")
        .output()
        .expect("run cinnabar");
    assert!(!output.status.success());

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("failed to read ROM"), "{stderr}");
}

#[test]
fn truncated_rom_prints_unknown_metadata() {
    let dir = temp_dir("truncated-rom");
    let path = dir.join("tiny.gba");
    fs::write(&path, [0u8; 16]).expect("write truncated rom");

    let output = Command::new(env!("CARGO_BIN_EXE_cinnabar"))
        .arg(&path)
        .arg("--info")
        .output()
        .expect("run cinnabar");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Title:       (unknown)"), "{stdout}");
    assert!(stdout.contains("ROM size:    16 bytes"), "{stdout}");
    assert!(stdout.contains("Header:      invalid"), "{stdout}");

    fs::remove_dir_all(&dir).expect("remove temp dir");
}

#[test]
fn debugger_persists_and_loads_a_battery_save() {
    let dir = temp_dir("debugger-save");
    let rom_path = dir.join("save-test.gba");
    let mut rom = cinnabar_testrom::demo_rom();
    rom[0x200..0x206].copy_from_slice(b"SRAM_V");
    fs::write(&rom_path, rom).expect("write save-test rom");
    let save_path = dir.join("custom.sav");

    let written = run_debugger(&rom_path, &save_path, "w8 0x0E000000 0xAB\nq\n");
    assert!(
        written.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&written.stderr)
    );
    let bytes = fs::read(&save_path).expect("read persisted save");
    assert_eq!(bytes.len(), 32 * 1024);
    assert_eq!(bytes[0], 0xAB);

    let loaded = run_debugger(&rom_path, &save_path, "x 0x0E000000 1\nq\n");
    assert!(
        loaded.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&loaded.stderr)
    );
    let stdout = String::from_utf8_lossy(&loaded.stdout);
    assert!(stdout.contains("0E000000: AB"), "{stdout}");

    fs::remove_dir_all(&dir).expect("remove temp dir");
}
