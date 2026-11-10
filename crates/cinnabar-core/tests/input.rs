use cinnabar_core::keypad::keys;
use cinnabar_core::{Cartridge, System};
use cinnabar_testrom::INPUT_ROM;

fn pixel(system: &System, x: usize, y: usize) -> u16 {
    system.framebuffer()[y * 240 + x]
}

#[test]
fn keypad_presses_light_up_the_indicators() {
    let mut system = System::new(Cartridge::new(INPUT_ROM.to_vec()));
    let _ = system.run_frames(3);

    // A button starts off, then turns red while Z is held.
    assert_eq!(pixel(&system, 188, 56), 0x1CE7);
    system.set_keys(keys::A);
    let _ = system.run_frames(2);
    assert_eq!(pixel(&system, 188, 56), 0x001F);
    system.set_keys(0);
    let _ = system.run_frames(2);
    assert_eq!(pixel(&system, 188, 56), 0x1CE7);

    // Right D-pad arm turns green.
    system.set_keys(keys::RIGHT);
    let _ = system.run_frames(2);
    assert_eq!(pixel(&system, 84, 76), 0x03E0);

    // Start turns white.
    system.set_keys(keys::START);
    let _ = system.run_frames(2);
    assert_eq!(pixel(&system, 100, 137), 0x7FFF);
}
