use cinnabar_core::{Cartridge, System};
use cinnabar_testrom::HALT_ROM;

#[test]
fn vblank_intr_wait_halts_and_wakes_each_frame() {
    let mut system = System::new(Cartridge::new(HALT_ROM.to_vec()));
    let output = system.run_frames(8);

    let frames = u32::from_le_bytes(system.bus.iwram[0..4].try_into().unwrap());
    assert!(frames > 0, "VBlankIntrWait never woke the CPU");
    assert_eq!(output.framebuffer[0], 0x7FFF, "IRQ handler did not run");
    assert!(
        system.bus.halted,
        "CPU should be waiting for the next VBlank"
    );
}
