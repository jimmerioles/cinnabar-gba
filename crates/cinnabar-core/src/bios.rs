//! High-level emulation of the GBA BIOS.
//!
//! Most commercial games reach the BIOS through SWI calls: decompression,
//! CpuSet, division, affine setup, interrupt waits. Cinnabar implements those
//! directly. No Nintendo BIOS image is required or distributed.

use crate::bus::{Bus, Memory};
use crate::cpu::Cpu;

pub fn handle_swi(bus: &mut Bus, cpu: &mut Cpu, number: u32) {
    bus.last_swi = number;
    match number {
        0x00 => soft_reset(bus),
        0x01 => register_ram_reset(bus, cpu.r[0]),
        0x02 | 0x03 => {
            bus.wait_irq = None;
            bus.wait_vblank = false;
            bus.halted = true;
        }
        0x04 => intr_wait(bus, cpu, cpu.r[0], cpu.r[1] as u16),
        0x05 => vblank_intr_wait(bus),
        0x06 => div(bus, cpu, false),
        0x07 => div(bus, cpu, true),
        0x08 => sqrt(cpu),
        0x09 => arc_tan(cpu),
        0x0A => arc_tan2(cpu),
        0x0B => cpu_set(bus, cpu),
        0x0C => cpu_fast_set(bus, cpu),
        0x0E => bg_affine_set(bus, cpu),
        0x0F => obj_affine_set(bus, cpu),
        0x10 => bit_unpack(bus, cpu),
        0x11 => lz77(bus, cpu, false),
        0x12 => lz77(bus, cpu, true),
        0x13 => huffman(bus, cpu),
        0x14 => rle(bus, cpu, false),
        0x15 => rle(bus, cpu, true),
        0x16 => diff8(bus, cpu, false),
        0x17 => diff8(bus, cpu, true),
        0x18 => diff16(bus, cpu),
        0x19 => {}
        0x1A..=0x1E | 0x20..=0x22 => {}
        0x1F => midi_key_to_freq(cpu),
        _ => {}
    }
}

fn soft_reset(bus: &mut Bus) {
    bus.reset_requested = true;
    bus.halted = true;
}

fn register_ram_reset(bus: &mut Bus, flags: u32) {
    if flags & (1 << 0) != 0 {
        bus.ewram.fill(0);
    }
    if flags & (1 << 1) != 0 {
        // The BIOS preserves the top 0x200 bytes of IWRAM for the system,
        // IRQ and BIOS stacks plus the user IRQ handler pointer. Games call
        // RegisterRamReset from their own stack, so wiping it would destroy
        // the caller's return address.
        bus.iwram[..0x7E00].fill(0);
    }
    if flags & (1 << 2) != 0 {
        bus.ppu.palette.fill(0);
    }
    if flags & (1 << 3) != 0 {
        bus.ppu.vram.fill(0);
    }
    if flags & (1 << 4) != 0 {
        bus.ppu.oam.fill(0);
    }
    if flags & (1 << 5) != 0 {
        bus.apu.write16(0x84 - 0x60, 0x0080);
    }
}

fn intr_wait(bus: &mut Bus, _cpu: &mut Cpu, discard: u32, mask: u16) {
    if discard != 0 {
        bus.irq.acknowledge(mask);
    }
    bus.wait_vblank = false;
    bus.wait_irq = Some(mask & 0x3FFF);
    bus.halted = true;
}

/// `VBlankIntrWait` waits for the next VBlank, not for the game to have
/// enabled the VBlank IRQ. Real hardware BIOS callers rely on this.
fn vblank_intr_wait(bus: &mut Bus) {
    bus.wait_irq = None;
    bus.wait_vblank = true;
    bus.halted = true;
}

fn div(bus: &mut Bus, cpu: &mut Cpu, arm_order: bool) {
    let (numerator, denominator) = if arm_order {
        (cpu.r[1] as i32, cpu.r[0] as i32)
    } else {
        (cpu.r[0] as i32, cpu.r[1] as i32)
    };
    let _ = bus;
    if denominator == 0 {
        cpu.r[0] = 0;
        cpu.r[1] = numerator as u32;
        cpu.r[3] = 0;
        return;
    }
    let quotient = numerator / denominator;
    let remainder = numerator % denominator;
    cpu.r[0] = quotient as u32;
    cpu.r[1] = remainder as u32;
    cpu.r[3] = quotient.unsigned_abs();
}

fn sqrt(cpu: &mut Cpu) {
    let value = cpu.r[0];
    let root = (value as f64).sqrt() as u32;
    cpu.r[0] = root;
    cpu.r[1] = value.wrapping_sub(root.wrapping_mul(root));
}

fn arc_tan(cpu: &mut Cpu) {
    let value = cpu.r[0] as i32 as f64 / 65536.0;
    let angle = value.atan() * 65536.0 / std::f64::consts::FRAC_PI_2;
    cpu.r[0] = angle as i32 as u32;
}

fn arc_tan2(cpu: &mut Cpu) {
    let x = cpu.r[0] as i32 as f64 / 65536.0;
    let y = cpu.r[1] as i32 as f64 / 65536.0;
    let angle = y.atan2(x) * 65536.0 / std::f64::consts::FRAC_PI_2;
    cpu.r[0] = angle as i32 as u32;
}

fn cpu_set(bus: &mut Bus, cpu: &mut Cpu) {
    let src = cpu.r[0];
    let dst = cpu.r[1];
    let control = cpu.r[2];
    let count = (control & 0x1F_FFFF) as usize;
    let word = control & (1 << 26) != 0;
    let fill = control & (1 << 24) != 0;

    if word {
        let value = if fill { bus.read32(src) } else { 0 };
        for i in 0..count {
            let target = dst.wrapping_add((i * 4) as u32);
            let value = if fill {
                value
            } else {
                bus.read32(src.wrapping_add((i * 4) as u32))
            };
            bus.write32(target, value);
        }
    } else {
        let value = if fill { bus.read16(src) as u32 } else { 0 };
        for i in 0..count {
            let target = dst.wrapping_add((i * 2) as u32);
            let value = if fill {
                value
            } else {
                bus.read16(src.wrapping_add((i * 2) as u32)) as u32
            };
            bus.write16(target, value as u16);
        }
    }
}

fn cpu_fast_set(bus: &mut Bus, cpu: &mut Cpu) {
    let src = cpu.r[0];
    let dst = cpu.r[1];
    let control = cpu.r[2];
    // CpuFastSet is always 32-bit; bit 24 is the fill flag. The count is a
    // word count (and must be a multiple of 8 on hardware).
    let words = (control & 0x1F_FFFF) as usize;
    let fill = control & (1 << 24) != 0;
    let value = if fill { bus.read32(src) } else { 0 };

    for i in 0..words {
        let offset = (i * 4) as u32;
        let value = if fill {
            value
        } else {
            bus.read32(src.wrapping_add(offset))
        };
        bus.write32(dst.wrapping_add(offset), value);
    }
}

fn read_u24(bus: &mut Bus, addr: &mut u32) -> usize {
    let value = bus.read8(*addr) as usize
        | ((bus.read8((*addr).wrapping_add(1)) as usize) << 8)
        | ((bus.read8((*addr).wrapping_add(2)) as usize) << 16);
    *addr = (*addr).wrapping_add(3);
    value
}

/// Write decompressed bytes to the destination. The VRAM variants of the BIOS
/// decompressors write 16-bit units, so an odd trailing byte is dropped.
fn write_output(bus: &mut Bus, dst: u32, bytes: &[u8], vram: bool) {
    if vram {
        for (index, pair) in bytes.chunks_exact(2).enumerate() {
            bus.write16(
                dst.wrapping_add((index * 2) as u32),
                u16::from_le_bytes([pair[0], pair[1]]),
            );
        }
    } else {
        for (index, &byte) in bytes.iter().enumerate() {
            bus.write8(dst.wrapping_add(index as u32), byte);
        }
    }
}

fn lz77(bus: &mut Bus, cpu: &mut Cpu, vram: bool) {
    let mut src = cpu.r[0];
    let dst = cpu.r[1];
    let header = bus.read8(src);
    src = src.wrapping_add(1);
    if header != 0x10 {
        return;
    }
    let size = read_u24(bus, &mut src);
    let mut written = 0usize;
    let mut window = Vec::with_capacity(size);

    while written < size {
        let flags = bus.read8(src);
        src = src.wrapping_add(1);
        // LZ77 flag bytes are read most-significant bit first.
        for bit in (0..8).rev() {
            if written >= size {
                break;
            }
            if flags & (1 << bit) != 0 {
                let b0 = bus.read8(src) as usize;
                let b1 = bus.read8(src + 1) as usize;
                src = src.wrapping_add(2);
                let length = ((b0 >> 4) & 0xF) + 3;
                let displacement = (((b0 & 0xF) << 8) | b1) + 1;
                for _ in 0..length {
                    if written >= size {
                        break;
                    }
                    let index = window.len().saturating_sub(displacement);
                    let value = window.get(index).copied().unwrap_or(0);
                    window.push(value);
                    written += 1;
                }
            } else {
                let value = bus.read8(src);
                src = src.wrapping_add(1);
                window.push(value);
                written += 1;
            }
        }
    }
    write_output(bus, dst, &window, vram);
}

fn rle(bus: &mut Bus, cpu: &mut Cpu, vram: bool) {
    let mut src = cpu.r[0];
    let dst = cpu.r[1];
    let header = bus.read8(src);
    src = src.wrapping_add(1);
    if header != 0x30 {
        return;
    }
    let size = read_u24(bus, &mut src);
    let mut written = 0usize;
    let mut output = Vec::with_capacity(size);

    while written < size {
        let flags = bus.read8(src);
        src = src.wrapping_add(1);
        // RLE flag bytes are read most-significant bit first.
        for bit in (0..8).rev() {
            if written >= size {
                break;
            }
            if flags & (1 << bit) != 0 {
                let value = bus.read8(src);
                let count = (bus.read8(src + 1) & 0x7F) as usize + 3;
                src = src.wrapping_add(2);
                for _ in 0..count {
                    if written >= size {
                        break;
                    }
                    output.push(value);
                    written += 1;
                }
            } else {
                let value = bus.read8(src);
                src = src.wrapping_add(1);
                output.push(value);
                written += 1;
            }
        }
    }
    write_output(bus, dst, &output, vram);
}

fn huffman(bus: &mut Bus, cpu: &mut Cpu) {
    // Huffman decompression is rarely used by games that don't also ship a
    // software decoder. Leave the destination untouched until it matters.
    let _ = (bus, cpu);
}

fn diff8(bus: &mut Bus, cpu: &mut Cpu, vram: bool) {
    let mut src = cpu.r[0];
    let dst = cpu.r[1];
    let header = bus.read8(src);
    src = src.wrapping_add(1);
    if header != 0x81 {
        return;
    }
    let size = read_u24(bus, &mut src);
    let mut value = 0u8;
    let mut output = Vec::with_capacity(size);
    for i in 0..size {
        value = value.wrapping_add(bus.read8(src.wrapping_add(i as u32)));
        output.push(value);
    }
    write_output(bus, dst, &output, vram);
}

fn diff16(bus: &mut Bus, cpu: &mut Cpu) {
    let mut src = cpu.r[0];
    let dst = cpu.r[1];
    let header = bus.read8(src);
    src = src.wrapping_add(1);
    if header != 0x82 {
        return;
    }
    let size = read_u24(bus, &mut src);
    let mut value = 0u16;
    for i in 0..(size / 2) {
        let delta = bus.read16(src.wrapping_add((i * 2) as u32));
        value = value.wrapping_add(delta);
        bus.write16(dst.wrapping_add((i * 2) as u32), value);
    }
}

fn bit_unpack(bus: &mut Bus, cpu: &mut Cpu) {
    let mut src = cpu.r[0];
    let mut dst = cpu.r[1];
    let info = cpu.r[2];
    let source_length = bus.read16(info) as u32;
    let source_width = bus.read8(info.wrapping_add(2));
    let dest_width = bus.read8(info.wrapping_add(3));
    let data_offset = bus.read32(info.wrapping_add(4));
    let zero_data = bus.read32(info.wrapping_add(8));

    if source_width == 0 || dest_width == 0 {
        return;
    }
    let mut out = 0u32;
    let mut bits = 0u32;
    let mut written_bytes = 0u32;

    for i in 0..source_length {
        let byte = bus.read8(src);
        src = src.wrapping_add(1);
        for bit in 0..8 {
            if i * 8 + bit >= source_length * 8 {
                break;
            }
            let value = (byte >> bit) & 1;
            let value = if value != 0 { data_offset } else { zero_data };
            out |= (value & ((1 << dest_width) - 1)) << bits;
            bits += dest_width as u32;
            while bits >= 8 {
                bus.write8(dst, out as u8);
                dst = dst.wrapping_add(1);
                out >>= 8;
                bits -= 8;
                written_bytes += 1;
            }
        }
    }
    let _ = written_bytes;
}

fn bg_affine_set(bus: &mut Bus, cpu: &mut Cpu) {
    let src = cpu.r[0];
    let dst = cpu.r[1];
    let count = cpu.r[2];

    for i in 0..count {
        let s = src.wrapping_add(i * 20);
        let d = dst.wrapping_add(i * 16);
        let cx = bus.read16(s) as i16 as i32;
        let cy = bus.read16(s + 2) as i16 as i32;
        let dx = bus.read16(s + 4) as i16 as i32;
        let dy = bus.read16(s + 6) as i16 as i32;
        let sx = bus.read16(s + 8) as i16 as i32;
        let sy = bus.read16(s + 10) as i16 as i32;
        let angle = bus.read16(s + 12);
        let cos = cosine(angle);
        let sin = sine(angle);

        let pa = (sx * cos) >> 14;
        let pb = (sx * -sin) >> 14;
        let pc = (sy * sin) >> 14;
        let pd = (sy * cos) >> 14;
        let x = cx - ((pa * dx + pb * dy) >> 8);
        let y = cy - ((pc * dx + pd * dy) >> 8);

        bus.write16(d, pa as u16);
        bus.write16(d + 2, pb as u16);
        bus.write16(d + 4, pc as u16);
        bus.write16(d + 6, pd as u16);
        bus.write32(d + 8, x as u32);
        bus.write32(d + 12, y as u32);
    }
}

fn obj_affine_set(bus: &mut Bus, cpu: &mut Cpu) {
    let src = cpu.r[0];
    let dst = cpu.r[1];
    let count = cpu.r[2];
    let stride = cpu.r[3];

    for i in 0..count {
        let s = src.wrapping_add(i * stride);
        let d = dst.wrapping_add(i * 8);
        let sx = bus.read16(s) as i16 as i32;
        let sy = bus.read16(s + 2) as i16 as i32;
        let angle = bus.read16(s + 4);
        let cos = cosine(angle);
        let sin = sine(angle);

        bus.write16(d, ((sx * cos) >> 14) as u16);
        bus.write16(d + 2, ((sx * -sin) >> 14) as u16);
        bus.write16(d + 4, ((sy * sin) >> 14) as u16);
        bus.write16(d + 6, ((sy * cos) >> 14) as u16);
    }
}

fn cosine(angle: u16) -> i32 {
    let radians = (angle as f64) * std::f64::consts::TAU / 65536.0;
    (radians.cos() * 16384.0) as i32
}

fn sine(angle: u16) -> i32 {
    let radians = (angle as f64) * std::f64::consts::TAU / 65536.0;
    (radians.sin() * 16384.0) as i32
}

fn midi_key_to_freq(cpu: &mut Cpu) {
    let key = cpu.r[0] as f64;
    let fine = cpu.r[1] as f64 / 256.0;
    let frequency = 440.0 * 2f64.powf((key + fine - 69.0) / 12.0);
    cpu.r[0] = (frequency * 65536.0) as u32;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cart::Cartridge;

    #[test]
    fn register_ram_reset_preserves_the_reserved_top_of_iwram() {
        let mut bus = Bus::new(Cartridge::new(vec![0; 0xC0]));
        bus.iwram.fill(0xAA);
        let mut cpu = Cpu::new();
        cpu.r[0] = 0x02;

        handle_swi(&mut bus, &mut cpu, 0x01);

        assert!(bus.iwram[..0x7E00].iter().all(|&byte| byte == 0));
        assert!(bus.iwram[0x7E00..].iter().all(|&byte| byte == 0xAA));
    }

    #[test]
    fn lz77_reads_flag_bits_most_significant_first() {
        let mut bus = Bus::new(Cartridge::new(vec![0; 0xC0]));
        let compressed = [0x10, 0x04, 0x00, 0x00, 0x40, b'A', 0x00, 0x00];
        for (index, &byte) in compressed.iter().enumerate() {
            bus.write8(0x0200_0000 + index as u32, byte);
        }

        let mut cpu = Cpu::new();
        cpu.r[0] = 0x0200_0000;
        cpu.r[1] = 0x0200_0100;
        handle_swi(&mut bus, &mut cpu, 0x11);

        assert_eq!(&bus.ewram[0x100..0x104], b"AAAA");
    }

    #[test]
    fn rle_reads_flag_bits_most_significant_first() {
        let mut bus = Bus::new(Cartridge::new(vec![0; 0xC0]));
        let compressed = [0x30, 0x04, 0x00, 0x00, 0x80, b'A', 0x01];
        for (index, &byte) in compressed.iter().enumerate() {
            bus.write8(0x0200_0000 + index as u32, byte);
        }

        let mut cpu = Cpu::new();
        cpu.r[0] = 0x0200_0000;
        cpu.r[1] = 0x0200_0100;
        handle_swi(&mut bus, &mut cpu, 0x14);

        assert_eq!(&bus.ewram[0x100..0x104], b"AAAA");
    }

    #[test]
    fn cpu_set_uses_bit_26_for_word_size_and_bit_24_for_fill() {
        let mut bus = Bus::new(Cartridge::new(vec![0; 0xC0]));
        bus.write32(0x0200_0000, 0x1122_3344);
        bus.write32(0x0200_0004, 0x5566_7788);

        let mut cpu = Cpu::new();
        cpu.r[0] = 0x0200_0000;
        cpu.r[1] = 0x0200_0100;
        cpu.r[2] = (1 << 26) | 2; // 32-bit copy of two words
        handle_swi(&mut bus, &mut cpu, 0x0B);

        assert_eq!(bus.read32(0x0200_0100), 0x1122_3344);
        assert_eq!(bus.read32(0x0200_0104), 0x5566_7788);
    }
}
