use std::collections::VecDeque;

use cinnabar_core::bus::Memory;
use cinnabar_core::{Cartridge, System};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "test-roms/gba-tests/arm/arm.gba".to_string());
    let start = u32::from_str_radix(std::env::args().nth(2).as_deref().unwrap_or("08001E00"), 16)?;
    let end = u32::from_str_radix(std::env::args().nth(3).as_deref().unwrap_or("08001F60"), 16)?;
    let limit: usize = std::env::args()
        .nth(4)
        .and_then(|s| s.parse().ok())
        .unwrap_or(200);
    let warmup: u32 = std::env::args()
        .nth(5)
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);

    let cart = Cartridge::new(std::fs::read(path)?);
    let mut system = System::new(cart);
    for _ in 0..warmup {
        system.run_frame();
    }
    println!(
        "after warmup {warmup}: pc={:08X} vcount={} frame_count={}",
        system.cpu.pc(),
        system.bus.ppu.vcount,
        system.bus.ppu.frame_count
    );
    let mut printed = 0;
    let mut history = VecDeque::with_capacity(16);
    for _ in 0..100_000_000 {
        let pc = system.cpu.pc();
        if !(0x0800_0000..0x0800_1000).contains(&pc) {
            println!("left ROM: pc={pc:08X} history={history:X?}");
            let r = system.cpu.regs();
            println!(
                "regs r0={:08X} r1={:08X} r2={:08X} r3={:08X} r4={:08X} r5={:08X} r6={:08X} r7={:08X} r8={:08X} r9={:08X} r10={:08X} r11={:08X} r12={:08X} sp={:08X} lr={:08X} cpsr={:08X}",
                r[0],
                r[1],
                r[2],
                r[3],
                r[4],
                r[5],
                r[6],
                r[7],
                r[8],
                r[9],
                r[10],
                r[11],
                r[12],
                r[13],
                r[14],
                system.cpu.cpsr()
            );
            break;
        }
        history.push_back(pc);
        if history.len() > 16 {
            history.pop_front();
        }
        if (start..end).contains(&pc) && printed < limit {
            let op = system.bus.read32(pc);
            let r = system.cpu.regs();
            println!(
                "pc={pc:08X} op={op:08X} r0={:08X} r1={:08X} r2={:08X} r3={:08X} r4={:08X} r7={:08X} r11={:08X} r12={:08X} sp={:08X} lr={:08X} cpsr={:08X}",
                r[0],
                r[1],
                r[2],
                r[3],
                r[4],
                r[7],
                r[11],
                r[12],
                r[13],
                r[14],
                system.cpu.cpsr()
            );
            printed += 1;
        }
        system.step();
        if printed >= limit {
            break;
        }
    }
    Ok(())
}
