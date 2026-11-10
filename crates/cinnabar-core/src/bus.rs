//! System bus and the CPU's view of memory.

use crate::apu::Apu;
use crate::bios;
use crate::cart::{Cartridge, SaveType};
use crate::cpu::Cpu;
use crate::dma::Dma;
use crate::irq::{self, Irq};
use crate::keypad::Keypad;
use crate::ppu::Ppu;
use crate::save::SaveMedia;
use crate::timer::Timers;

/// The CPU's view of the machine. Implemented by [`Bus`] and by test harnesses.
pub trait Memory {
    fn read8(&mut self, addr: u32) -> u8;
    fn read16(&mut self, addr: u32) -> u16;
    fn read32(&mut self, addr: u32) -> u32;

    fn write8(&mut self, addr: u32, value: u8);
    fn write16(&mut self, addr: u32, value: u16);
    fn write32(&mut self, addr: u32, value: u32);

    /// Handle a software interrupt. The default is a no-op so CPU tests can
    /// ignore the BIOS entirely.
    fn swi(&mut self, _cpu: &mut Cpu, _number: u32) {}
}

/// The GBA IRQ trampoline, assembled from `asm/irq.s` with llvm-mc.
///
/// It saves the caller-saved registers, jumps through the user handler at
/// 0x0300_7FFC, then returns to the interrupted instruction while restoring
/// CPSR from SPSR (which also restores ARM/Thumb state).
const IRQ_STUB: [u8; 56] = [
    0x0F, 0x50, 0x2D, 0xE9, 0x03, 0x04, 0xA0, 0xE3, 0x7F, 0x0C, 0x80, 0xE2, 0xFC, 0x00,
    0x80, 0xE2, 0x00, 0x00, 0x90, 0xE5, 0x00, 0x00, 0x50, 0xE3, 0x01, 0x00, 0x00, 0x0A,
    0x0F, 0xE0, 0xA0, 0xE1, 0x10, 0xFF, 0x2F, 0xE1, 0x00, 0x00, 0x4F, 0xE1, 0x20, 0x00,
    0x10, 0xE3, 0x0F, 0x50, 0xBD, 0xE8, 0x02, 0xF0, 0x5E, 0x12, 0x04, 0xF0, 0x5E, 0xE2,
];

pub struct Bus {
    pub bios: Vec<u8>,
    pub ewram: Vec<u8>,
    pub iwram: Vec<u8>,

    pub cart: Cartridge,
    pub save: SaveMedia,
    pub ppu: Ppu,
    pub apu: Apu,
    pub dma: Dma,
    pub timers: Timers,
    pub irq: Irq,
    pub keypad: Keypad,

    pub waitcnt: u16,
    pub postflg: u8,
    pub halted: bool,
    pub wait_irq: Option<u16>,
    pub reset_requested: bool,
    pub last_swi: u32,

    open_bus: u32,
    dma_active: bool,
}

impl Bus {
    pub fn new(cart: Cartridge) -> Self {
        let save = SaveMedia::new(cart.save_type);
        let mut bios = vec![0u8; 0x4000];
        bios[0x18..0x18 + IRQ_STUB.len()].copy_from_slice(&IRQ_STUB);

        Self {
            bios,
            ewram: vec![0; 0x4_0000],
            iwram: vec![0; 0x8000],
            cart,
            save,
            ppu: Ppu::new(),
            apu: Apu::new(),
            dma: Dma::default(),
            timers: Timers::default(),
            irq: Irq::default(),
            keypad: Keypad::default(),
            waitcnt: 0,
            postflg: 0,
            halted: false,
            wait_irq: None,
            reset_requested: false,
            last_swi: 0,
            open_bus: 0,
            dma_active: false,
        }
    }

    pub fn set_keys(&mut self, pressed: u16) {
        let before = self.keypad.irq_condition();
        self.keypad.set_pressed(pressed);
        let after = self.keypad.irq_condition();
        if after && !before {
            self.irq.request(irq::bits::KEYPAD);
        }
    }

    pub fn step(&mut self, cycles: u32) {
        self.timers.step(cycles, &mut self.irq);
        self.apu.step(cycles);
        let events = self.ppu.step(cycles, &mut self.irq);
        if events.hblank {
            self.run_dma_timing(2);
        }
        if events.vblank {
            self.run_dma_timing(1);
        }
    }

    fn run_dma_timing(&mut self, timing: u16) {
        for channel in 0..4 {
            let ch = &self.dma.channels[channel];
            if ch.enabled() && ch.timing() == timing {
                self.run_dma_channel(channel);
            }
        }
    }

    pub fn run_dma_channel(&mut self, index: usize) {
        if self.dma_active || index >= 4 {
            return;
        }
        let channel = self.dma.channels[index].clone();
        if !channel.enabled() {
            return;
        }

        self.dma_active = true;
        let count = channel.count_value(index);
        let word = channel.word_size();
        let mut src = channel.src;
        let mut dst = channel.dst;

        for _ in 0..count {
            if word {
                let value = self.read32(src);
                self.write32(dst, value);
                src = adjust_address(src, channel.src_control(), 4);
                dst = adjust_address(dst, channel.dest_control(), 4);
            } else {
                let value = self.read16(src);
                self.write16(dst, value);
                src = adjust_address(src, channel.src_control(), 2);
                dst = adjust_address(dst, channel.dest_control(), 2);
            }
        }

        let repeat = channel.repeat() && channel.timing() != 0;
        let ch = &mut self.dma.channels[index];
        if repeat {
            ch.src = channel.src;
            ch.dst = channel.dst;
            ch.count = channel.count;
            if channel.dest_control() == 3 {
                ch.dst = channel.dst;
            }
        } else {
            ch.control &= !0x8000;
        }

        if channel.irq_enabled() {
            self.irq.request(0x100 << index);
        }
        self.dma_active = false;
    }

    fn read_io16(&mut self, offset: u16) -> u16 {
        match offset {
            0x000..=0x05F => self.ppu.read_io16(offset),
            0x060..=0x0A7 => self.apu.read16(offset - 0x060),
            0x0B0..=0x0DF => self.dma.read16(offset),
            0x100..=0x10F => {
                let index = ((offset - 0x100) / 4) as usize;
                match (offset - 0x100) % 4 {
                    0 => self.timers.read_reload(index),
                    2 => self.timers.read_control(index),
                    _ => 0,
                }
            }
            0x130 => self.keypad.read(),
            0x132 => self.keypad.keycnt,
            0x200 => self.irq.ie,
            0x202 => self.irq.if_,
            0x204 => self.waitcnt,
            0x208 => self.irq.ime as u16,
            0x300 => self.postflg as u16,
            _ => 0,
        }
    }

    fn write_io16(&mut self, offset: u16, value: u16) {
        match offset {
            0x000..=0x05F => self.ppu.write_io16(offset, value),
            0x060..=0x0A7 => self.apu.write16(offset - 0x060, value),
            0x0B0..=0x0DF => {
                self.dma.write16(offset, value);
                if let Some(index) = Dma::channel_count(offset) {
                    let ch = &self.dma.channels[index];
                    if ch.enabled() && ch.timing() == 0 {
                        self.run_dma_channel(index);
                    }
                }
            }
            0x100..=0x10F => {
                let index = ((offset - 0x100) / 4) as usize;
                match (offset - 0x100) % 4 {
                    0 => self.timers.write_reload(index, value),
                    2 => self.timers.write_control(index, value),
                    _ => {}
                }
            }
            0x132 => self.keypad.keycnt = value,
            0x200 => self.irq.ie = value & 0x3FFF,
            0x202 => self.irq.acknowledge(value),
            0x204 => self.waitcnt = value,
            0x208 => self.irq.ime = value & 1 != 0,
            0x300 => self.postflg = value as u8,
            _ => {}
        }
    }

    fn read_io8(&mut self, offset: u16) -> u8 {
        match offset {
            0x300 => self.postflg,
            0x301 => 0,
            _ => {
                let value = self.read_io16(offset & !1);
                if offset & 1 == 0 {
                    value as u8
                } else {
                    (value >> 8) as u8
                }
            }
        }
    }

    fn write_io8(&mut self, offset: u16, value: u8) {
        match offset {
            0x300 => self.postflg = value,
            0x301 => {
                if value == 0 {
                    self.halted = true;
                }
            }
            _ => {
                let old = self.read_io16(offset & !1);
                let new = if offset & 1 == 0 {
                    (old & 0xFF00) | value as u16
                } else {
                    (old & 0x00FF) | ((value as u16) << 8)
                };
                self.write_io16(offset & !1, new);
            }
        }
    }
}

impl Memory for Bus {
    fn read8(&mut self, addr: u32) -> u8 {
        let value = match addr >> 24 {
            0x00 => self.bios[(addr as usize) & 0x3FFF],
            0x02 => self.ewram[(addr as usize) & 0x3_FFFF],
            0x03 => self.iwram[(addr as usize) & 0x7FFF],
            0x04 => self.read_io8((addr & 0x3FF) as u16),
            0x05 => self.ppu.read_palette8(addr),
            0x06 => self.ppu.read_vram8(addr),
            0x07 => self.ppu.read_oam8(addr),
            0x08..=0x0D => self.cart.read8(addr),
            0x0E | 0x0F => self.save.read8(addr),
            _ => (self.open_bus >> ((addr & 3) * 8)) as u8,
        };
        self.open_bus = (self.open_bus & 0xFFFF_FF00) | value as u32;
        value
    }

    fn read16(&mut self, addr: u32) -> u16 {
        let value = match addr >> 24 {
            0x00 => read16_slice(&self.bios, addr as usize),
            0x02 => read16_slice(&self.ewram, addr as usize),
            0x03 => read16_slice(&self.iwram, addr as usize),
            0x04 => self.read_io16((addr & 0x3FF) as u16),
            0x05 => self.ppu.read_palette16(addr),
            0x06 => self.ppu.read_vram16(addr),
            0x07 => self.ppu.read_oam16(addr),
            0x08..=0x0D => self.cart.read16(addr),
            0x0E | 0x0F => self.save.read16(addr),
            _ => self.open_bus as u16,
        };
        self.open_bus = (self.open_bus & 0xFFFF_0000) | value as u32;
        value
    }

    fn read32(&mut self, addr: u32) -> u32 {
        let value = match addr >> 24 {
            0x00 => read32_slice(&self.bios, addr as usize),
            0x02 => read32_slice(&self.ewram, addr as usize),
            0x03 => read32_slice(&self.iwram, addr as usize),
            0x04 => self.read_io32((addr & 0x3FF) as u16),
            0x05 => self.ppu.read_palette16(addr) as u32
                | ((self.ppu.read_palette16(addr.wrapping_add(2)) as u32) << 16),
            0x06 => self.ppu.read_vram32(addr),
            0x07 => self.ppu.read_oam32(addr),
            0x08..=0x0D => self.cart.read32(addr),
            0x0E | 0x0F => self.save.read32(addr),
            _ => self.open_bus,
        };
        self.open_bus = value;
        value
    }

    fn write8(&mut self, addr: u32, value: u8) {
        match addr >> 24 {
            0x00 => {}
            0x02 => self.ewram[(addr as usize) & 0x3_FFFF] = value,
            0x03 => self.iwram[(addr as usize) & 0x7FFF] = value,
            0x04 => self.write_io8((addr & 0x3FF) as u16, value),
            0x05 => self.ppu.write_palette8(addr, value),
            0x06 => self.ppu.write_vram8(addr, value),
            0x07 => self.ppu.write_oam8(addr, value),
            0x08..=0x0D => {}
            0x0E | 0x0F => self.save.write8(addr, value),
            _ => {}
        }
    }

    fn write16(&mut self, addr: u32, value: u16) {
        match addr >> 24 {
            0x00 => {}
            0x02 => write16_slice(&mut self.ewram, (addr & !1) as usize, value),
            0x03 => write16_slice(&mut self.iwram, (addr & !1) as usize, value),
            0x04 => self.write_io16(((addr & 0x3FF) & !1) as u16, value),
            0x05 => self.ppu.write_palette16(addr & !1, value),
            0x06 => self.ppu.write_vram16(addr & !1, value),
            0x07 => self.ppu.write_oam16(addr & !1, value),
            0x08..=0x0D => {}
            // Save media is an 8-bit bus; preserve the byte lane.
            0x0E | 0x0F => self.save.write16(addr, value),
            _ => {}
        }
    }

    fn write32(&mut self, addr: u32, value: u32) {
        match addr >> 24 {
            0x00 => {}
            0x02 => write32_slice(&mut self.ewram, (addr & !3) as usize, value),
            0x03 => write32_slice(&mut self.iwram, (addr & !3) as usize, value),
            0x04 => self.write_io32(((addr & 0x3FF) & !3) as u16, value),
            0x05 => self.ppu.write_palette32(addr & !3, value),
            0x06 => self.ppu.write_vram32(addr & !3, value),
            0x07 => self.ppu.write_oam32(addr & !3, value),
            0x08..=0x0D => {}
            0x0E | 0x0F => self.save.write32(addr, value),
            _ => {}
        }
    }

    fn swi(&mut self, cpu: &mut Cpu, number: u32) {
        bios::handle_swi(self, cpu, number);
    }
}

impl Bus {
    pub fn read_io32(&mut self, offset: u16) -> u32 {
        match offset {
            0x028..=0x03F => self.ppu.read_io32(offset),
            _ => self.read_io16(offset) as u32
                | ((self.read_io16(offset.wrapping_add(2)) as u32) << 16),
        }
    }

    pub fn write_io32(&mut self, offset: u16, value: u32) {
        match offset {
            0x028..=0x03F => self.ppu.write_io32(offset, value),
            _ => {
                self.write_io16(offset, value as u16);
                self.write_io16(offset.wrapping_add(2), (value >> 16) as u16);
            }
        }
    }

    pub fn save_type(&self) -> SaveType {
        self.cart.save_type
    }
}

fn adjust_address(addr: u32, control: u16, size: u32) -> u32 {
    match control {
        0 | 3 => addr.wrapping_add(size),
        1 => addr.wrapping_sub(size),
        _ => addr,
    }
}

fn read16_slice(slice: &[u8], addr: usize) -> u16 {
    let offset = addr & (slice.len() - 1);
    u16::from_le_bytes([slice[offset], slice[(offset + 1) & (slice.len() - 1)]])
}

fn read32_slice(slice: &[u8], addr: usize) -> u32 {
    read16_slice(slice, addr) as u32 | ((read16_slice(slice, addr + 2) as u32) << 16)
}

fn write16_slice(slice: &mut [u8], addr: usize, value: u16) {
    let offset = addr & (slice.len() - 1);
    let bytes = value.to_le_bytes();
    slice[offset] = bytes[0];
    slice[(offset + 1) & (slice.len() - 1)] = bytes[1];
}

fn write32_slice(slice: &mut [u8], addr: usize, value: u32) {
    write16_slice(slice, addr, value as u16);
    write16_slice(slice, addr + 2, (value >> 16) as u16);
}
