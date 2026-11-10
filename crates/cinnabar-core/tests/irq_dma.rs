use cinnabar_core::{Cartridge, System};
use cinnabar_testrom::IRQ_DMA_ROM;

#[test]
fn dma_fills_vram_and_timer_vblank_irqs_run() {
    let mut system = System::new(Cartridge::new(IRQ_DMA_ROM.to_vec()));
    let output = system.run_frames(8);

    // The DMA3 fill paints the whole mode-3 screen dark blue.
    assert_eq!(output.framebuffer[1], 0x0C00, "DMA fill should cover VRAM");

    // The timer IRQ handler paints pixel 0 green. If either the timer IRQ or
    // the BIOS IRQ trampoline is broken, this stays dark blue.
    assert_eq!(
        output.framebuffer[0], 0x03E0,
        "timer IRQ should write green"
    );

    // The handler increments a VBlank counter and a timer counter in IWRAM.
    let vblank = u32::from_le_bytes(system.bus.iwram[0..4].try_into().unwrap());
    let timer = u32::from_le_bytes(system.bus.iwram[4..8].try_into().unwrap());
    assert!(vblank > 0, "VBlank IRQ did not fire");
    assert!(timer > 0, "timer IRQ did not fire");
    assert_eq!(
        system.bus.irq.if_, 0,
        "handler should acknowledge both IRQs"
    );
}
