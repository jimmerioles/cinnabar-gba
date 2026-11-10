use cinnabar_core::bus::Memory;
use cinnabar_core::{Cartridge, System};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "test-roms/gba-tests/arm/arm.gba".to_string());
    let start = u32::from_str_radix(
        std::env::args().nth(2).as_deref().unwrap_or("08001E00"),
        16,
    )?;
    let end = u32::from_str_radix(
        std::env::args().nth(3).as_deref().unwrap_or("08001F60"),
        16,
    )?;
    let limit: usize = std::env::args()
        .nth(4)
        .and_then(|s| s.parse().ok())
        .unwrap_or(200);

    let cart = Cartridge::new(std::fs::read(path)?);
    let mut system = System::new(cart);
    let mut printed = 0;
    for _ in 0..4_000_000 {
        let pc = system.cpu.pc();
        if (start..end).contains(&pc) && printed < limit {
            let op = system.bus.read32(pc);
            let r = system.cpu.regs();
            println!(
                "pc={pc:08X} op={op:08X} r0={:08X} r1={:08X} r2={:08X} r3={:08X} r4={:08X} r11={:08X} r12={:08X} sp={:08X} lr={:08X} cpsr={:08X}",
                r[0], r[1], r[2], r[3], r[4], r[11], r[12], r[13], r[14], system.cpu.cpsr()
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
