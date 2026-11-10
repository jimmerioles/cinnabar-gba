use cinnabar_core::{Cartridge, System};
use cinnabar_testrom::TILED_ROM;

#[test]
fn tiled_mode_renders_tiles_sprites_and_blending() {
    let mut system = System::new(Cartridge::new(TILED_ROM.to_vec()));
    let output = system.run_frames(3);

    let colors = output
        .framebuffer
        .iter()
        .copied()
        .collect::<std::collections::BTreeSet<_>>();
    assert!(
        colors.len() >= 5,
        "expected tiles, a sprite and a blend, got {} colors",
        colors.len()
    );

    // The 8x8 sprite sits at (104, 64) over a white tile and is alpha-blended
    // with it, so the pixel must be visible but not the raw sprite color.
    let sprite = output.framebuffer[64 * 240 + 104];
    assert_ne!(sprite, 0, "sprite pixel should be visible");
    assert_ne!(
        sprite, 0x001F,
        "sprite pixel should be alpha-blended, got {sprite:04X}"
    );
}
