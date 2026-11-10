//! Versioned save-state snapshots.
//!
//! A snapshot contains the mutable machine state only: CPU, work RAM, save
//! media, PPU, APU, DMA, timers, interrupts, and the bus latches. The
//! cartridge ROM and the generated BIOS are not duplicated; loading validates
//! that the snapshot belongs to the same cartridge.

use std::fmt;

use serde::{Deserialize, Serialize};

use crate::apu::Apu;
use crate::cpu::Cpu;
use crate::dma::Dma;
use crate::irq::Irq;
use crate::keypad::Keypad;
use crate::ppu::Ppu;
use crate::save::SaveMedia;
use crate::timer::Timers;

pub const SAVE_STATE_MAGIC: [u8; 4] = *b"CINN";
pub const SAVE_STATE_VERSION: u16 = 1;

#[derive(Clone, Serialize, Deserialize)]
pub(crate) struct SaveState {
    pub(crate) magic: [u8; 4],
    pub(crate) version: u16,
    pub(crate) rom_size: usize,
    pub(crate) title: String,
    pub(crate) game_code: String,
    pub(crate) cpu: Cpu,
    pub(crate) bus: BusState,
    pub(crate) frame_cycles: u64,
}

#[derive(Clone, Serialize, Deserialize)]
pub(crate) struct BusState {
    pub(crate) ewram: Vec<u8>,
    pub(crate) iwram: Vec<u8>,
    pub(crate) save: SaveMedia,
    pub(crate) ppu: Ppu,
    pub(crate) apu: Apu,
    pub(crate) dma: Dma,
    pub(crate) timers: Timers,
    pub(crate) irq: Irq,
    pub(crate) keypad: Keypad,
    pub(crate) waitcnt: u16,
    pub(crate) postflg: u8,
    pub(crate) halted: bool,
    pub(crate) wait_irq: Option<u16>,
    pub(crate) wait_vblank: bool,
    pub(crate) reset_requested: bool,
    pub(crate) pending_cycles: u32,
    pub(crate) last_swi: u32,
    pub(crate) open_bus: u32,
    pub(crate) dma_active: bool,
}

#[derive(Debug)]
pub enum SaveStateError {
    Encode(bincode::Error),
    Decode(bincode::Error),
    BadMagic,
    UnsupportedVersion(u16),
    WrongCartridge,
}

impl fmt::Display for SaveStateError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Encode(error) => write!(formatter, "could not encode save state: {error}"),
            Self::Decode(error) => write!(formatter, "could not decode save state: {error}"),
            Self::BadMagic => write!(formatter, "not a Cinnabar save state"),
            Self::UnsupportedVersion(version) => {
                write!(formatter, "unsupported save-state version {version}")
            }
            Self::WrongCartridge => {
                write!(formatter, "save state belongs to a different cartridge")
            }
        }
    }
}

impl std::error::Error for SaveStateError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Encode(error) | Self::Decode(error) => Some(error),
            _ => None,
        }
    }
}

impl From<bincode::Error> for SaveStateError {
    fn from(error: bincode::Error) -> Self {
        Self::Decode(error)
    }
}
