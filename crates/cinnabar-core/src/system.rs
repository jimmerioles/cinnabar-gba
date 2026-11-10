//! The machine: CPU + bus, driven one frame at a time.

use crate::bus::Bus;
use crate::cart::Cartridge;
use crate::cpu::Cpu;
use crate::save::SaveMedia;

#[derive(Clone, Debug, Default)]
pub struct FrameOutput {
    pub framebuffer: Vec<u16>,
    pub audio: Vec<f32>,
}

pub struct System {
    pub cpu: Cpu,
    pub bus: Bus,
    frame_cycles: u64,
}

impl System {
    pub fn new(cart: Cartridge) -> Self {
        let mut cpu = Cpu::new();
        cpu.reset_to_cart();
        let bus = Bus::new(cart);
        Self {
            cpu,
            bus,
            frame_cycles: 0,
        }
    }

    pub fn reset(&mut self) {
        let cart = self.bus.cart.clone();
        let save: SaveMedia = std::mem::replace(
            &mut self.bus.save,
            SaveMedia::new(crate::cart::SaveType::None),
        );
        let mut bus = Bus::new(cart);
        bus.save = save;
        self.bus = bus;
        self.cpu = Cpu::new();
        self.cpu.reset_to_cart();
        self.frame_cycles = 0;
    }

    pub fn set_keys(&mut self, pressed: u16) {
        self.bus.set_keys(pressed);
    }

    pub fn run_frame(&mut self) -> FrameOutput {
        self.bus.ppu.frame_ready = false;
        let mut guard = 0u32;
        while !self.bus.ppu.frame_ready {
            self.step();
            guard += 1;
            if guard > 4_000_000 {
                // A hung CPU must not hang the frontend. Returning a partial
                // frame is better than freezing the window.
                break;
            }
        }

        if self.bus.reset_requested {
            self.reset();
        }

        FrameOutput {
            framebuffer: self.bus.ppu.framebuffer.clone(),
            audio: self.bus.apu.take_samples(),
        }
    }

    pub fn step(&mut self) {
        if self.bus.halted {
            self.bus.step(1);
            self.frame_cycles = self.frame_cycles.wrapping_add(1);
            let wake = match self.bus.wait_irq {
                Some(mask) => self.bus.irq.if_ & mask != 0,
                None => self.bus.irq.ie & self.bus.irq.if_ != 0,
            };
            if wake {
                self.bus.halted = false;
                self.bus.wait_irq = None;
            }
            return;
        }

        let cycles = self.cpu.step(&mut self.bus);
        self.bus.step(cycles);
        self.frame_cycles = self.frame_cycles.wrapping_add(cycles as u64);
        self.dispatch_irq();
    }

    fn dispatch_irq(&mut self) {
        if self.bus.irq.pending() && self.cpu.irq_enabled() {
            self.cpu.irq();
        }
    }

    pub fn frame_cycles(&self) -> u64 {
        self.frame_cycles
    }

    pub fn framebuffer(&self) -> &[u16] {
        &self.bus.ppu.framebuffer
    }

    pub fn run_frames(&mut self, frames: u32) -> FrameOutput {
        let mut output = FrameOutput::default();
        for _ in 0..frames {
            output = self.run_frame();
        }
        output
    }
}

