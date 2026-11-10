use cinnabar_core::{Cartridge, System};

fn main() {
    let rom = include_bytes!("../tests/fixtures/demo.gba").to_vec();
    let cart = Cartridge::new(rom);
    let mut system = System::new(cart);

    for frame in 0..4 {
        let output = system.run_frame();
        println!(
            "frame {frame}: pc={:08X} cpsr={:08X} r2={:08X} r3={:08X} r4={:08X} r5={:08X} vcount={} line_cycles={} frames={} dispcnt={:04X}",
            system.cpu.pc(),
            system.cpu.cpsr(),
            system.cpu.regs()[2],
            system.cpu.regs()[3],
            system.cpu.regs()[4],
            system.cpu.regs()[5],
            system.bus.ppu.vcount,
            system.bus.ppu.line_cycles(),
            system.bus.ppu.frame_count,
            system.bus.ppu.dispcnt,
        );
        println!(
            "  vram: {:02X}{:02X} {:02X}{:02X} {:02X}{:02X} {:02X}{:02X} {:02X}{:02X} {:02X}{:02X} {:02X}{:02X} {:02X}{:02X}",
            system.bus.ppu.vram[0],
            system.bus.ppu.vram[1],
            system.bus.ppu.vram[2],
            system.bus.ppu.vram[3],
            system.bus.ppu.vram[4],
            system.bus.ppu.vram[5],
            system.bus.ppu.vram[6],
            system.bus.ppu.vram[7],
            system.bus.ppu.vram[8],
            system.bus.ppu.vram[9],
            system.bus.ppu.vram[10],
            system.bus.ppu.vram[11],
            system.bus.ppu.vram[12],
            system.bus.ppu.vram[13],
            system.bus.ppu.vram[14],
            system.bus.ppu.vram[15],
        );
        for row in [0usize, 1, 2, 10, 159] {
            let start = row * 240;
            println!(
                "  row {row:3}: {:04X} {:04X} {:04X} {:04X} {:04X} {:04X} {:04X} {:04X}",
                output.framebuffer[start],
                output.framebuffer[start + 1],
                output.framebuffer[start + 2],
                output.framebuffer[start + 3],
                output.framebuffer[start + 4],
                output.framebuffer[start + 5],
                output.framebuffer[start + 6],
                output.framebuffer[start + 7],
            );
        }
    }
}
