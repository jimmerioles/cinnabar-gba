//! End-to-end checks of the `cinnabar` binary's CI paths.

use std::fs;
use std::path::PathBuf;
use std::process::Command;
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
