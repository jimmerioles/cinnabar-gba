//! The machine: CPU + bus, driven one frame at a time.

use crate::bus::{Bus, Memory};
use crate::cart::Cartridge;
use crate::cpu::Cpu;
use crate::save::SaveMedia;
use crate::save_state::{SAVE_STATE_MAGIC, SAVE_STATE_VERSION, SaveState, SaveStateError};

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
            let vblank_before = self.bus.ppu.vblank_count;
            self.bus.step(1);
            self.frame_cycles = self.frame_cycles.wrapping_add(1);
            let wake = if self.bus.wait_vblank {
                self.bus.ppu.vblank_count != vblank_before
            } else {
                match self.bus.wait_irq {
                    Some(mask) => self.bus.irq.if_ & mask != 0,
                    None => self.bus.irq.ie & self.bus.irq.if_ != 0,
                }
            };
            if wake {
                self.bus.halted = false;
                self.bus.wait_irq = None;
                self.bus.wait_vblank = false;
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

    /// Serialize the complete mutable machine state.
    pub fn save_state(&self) -> Result<Vec<u8>, SaveStateError> {
        let state = SaveState {
            magic: SAVE_STATE_MAGIC,
            version: SAVE_STATE_VERSION,
            rom_size: self.bus.cart.size(),
            title: self.bus.cart.title.clone(),
            game_code: self.bus.cart.game_code.clone(),
            cpu: self.cpu.clone(),
            bus: self.bus.save_state(),
            frame_cycles: self.frame_cycles,
        };
        bincode::serialize(&state).map_err(SaveStateError::Encode)
    }

    /// Restore a snapshot produced by [`System::save_state`].
    pub fn load_state(&mut self, bytes: &[u8]) -> Result<(), SaveStateError> {
        let state: SaveState = bincode::deserialize(bytes).map_err(SaveStateError::Decode)?;
        if state.magic != SAVE_STATE_MAGIC {
            return Err(SaveStateError::BadMagic);
        }
        if state.version != SAVE_STATE_VERSION {
            return Err(SaveStateError::UnsupportedVersion(state.version));
        }
        if state.rom_size != self.bus.cart.size()
            || state.title != self.bus.cart.title
            || state.game_code != self.bus.cart.game_code
        {
            return Err(SaveStateError::WrongCartridge);
        }

        self.cpu = state.cpu;
        self.bus.restore_state(state.bus);
        self.frame_cycles = state.frame_cycles;
        Ok(())
    }

    /// Write memory with normal hardware side effects, for debugger edits.
    pub fn poke8(&mut self, addr: u32, value: u8) {
        self.bus.write8(addr, value);
    }

    /// Write memory with normal hardware side effects, for debugger edits.
    pub fn poke16(&mut self, addr: u32, value: u16) {
        self.bus.write16(addr, value);
    }

    /// Write memory with normal hardware side effects, for debugger edits.
    pub fn poke32(&mut self, addr: u32, value: u32) {
        self.bus.write32(addr, value);
    }

    /// Read memory without side effects, for debugger views.
    pub fn peek8(&self, addr: u32) -> u8 {
        self.bus.peek8(addr)
    }

    /// Read memory without side effects, for debugger views.
    pub fn peek16(&self, addr: u32) -> u16 {
        self.bus.peek16(addr)
    }

    /// Read memory without side effects, for debugger views.
    pub fn peek32(&self, addr: u32) -> u32 {
        self.bus.peek32(addr)
    }

    pub fn run_frames(&mut self, frames: u32) -> FrameOutput {
        let mut output = FrameOutput::default();
        for _ in 0..frames {
            output = self.run_frame();
        }
        output
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Cartridge;

    fn demo() -> System {
        System::new(Cartridge::new(cinnabar_testrom::demo_rom()))
    }

    #[test]
    fn save_state_round_trips_the_cpu_and_ppu() {
        let mut system = demo();
        for _ in 0..5 {
            let _ = system.run_frame();
        }
        let state = system.save_state().expect("encode state");
        let pc = system.cpu.pc();
        let frame = system.bus.ppu.frame_count;
        let vram = system.bus.ppu.vram.clone();

        for _ in 0..5 {
            let _ = system.run_frame();
        }
        assert_ne!(system.bus.ppu.frame_count, frame);

        system.load_state(&state).expect("decode state");
        assert_eq!(system.cpu.pc(), pc);
        assert_eq!(system.bus.ppu.frame_count, frame);
        assert_eq!(system.bus.ppu.vram, vram);
    }

    #[test]
    fn save_state_rejects_a_different_cartridge() {
        let source = demo();
        let state = source.save_state().expect("encode state");
        let mut other = System::new(Cartridge::new(vec![0; 0x8000]));
        assert!(matches!(
            other.load_state(&state),
            Err(SaveStateError::WrongCartridge)
        ));
    }

    #[test]
    fn pokes_write_ram_and_io() {
        let mut system = demo();
        system.poke32(0x0200_0000, 0x1234_5678);
        assert_eq!(system.peek32(0x0200_0000), 0x1234_5678);
        system.poke16(0x0400_0000, 0x1F00);
        assert_eq!(system.bus.ppu.dispcnt, 0x1F00);
    }

    #[test]
    fn save_state_rejects_bad_magic() {
        let mut system = demo();
        let mut state = system.save_state().expect("encode state");
        state[0] ^= 0xFF;
        assert!(matches!(
            system.load_state(&state),
            Err(SaveStateError::BadMagic)
        ));
    }
}
