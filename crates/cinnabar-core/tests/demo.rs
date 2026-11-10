use cinnabar_core::{Cartridge, System};

fn demo_system() -> System {
    let rom = include_bytes!("fixtures/demo.gba").to_vec();
    let cart = Cartridge::new(rom);
    System::new(cart)
}

#[test]
fn demo_rom_boots_and_renders() {
    let mut system = demo_system();
    let output = system.run_frames(4);

    assert_eq!(output.framebuffer.len(), 240 * 160);

    // The demo fills VRAM with an incrementing BGR555 value. These pixels
    // prove the CPU loop ran and mode 3 is being displayed.
    assert_eq!(output.framebuffer[0], 0);
    assert_eq!(output.framebuffer[1], 1);
    assert_eq!(output.framebuffer[2], 2);
    assert_eq!(output.framebuffer[240], 240);

    let distinct = output
        .framebuffer
        .iter()
        .copied()
        .collect::<std::collections::BTreeSet<_>>()
        .len();
    assert!(distinct > 1000, "expected a gradient, got {distinct} colors");
}

#[test]
fn cartridge_header_is_parsed() {
    let rom = include_bytes!("fixtures/demo.gba").to_vec();
    let cart = Cartridge::new(rom);
    assert_eq!(cart.title, "CINNABAR");
    assert_eq!(cart.game_code, "CINB");
}
