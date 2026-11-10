//! Direct Memory Access channels.

#[derive(Clone, Debug, Default)]
pub struct DmaChannel {
    pub src: u32,
    pub dst: u32,
    pub count: u16,
    pub control: u16,
}

impl DmaChannel {
    pub fn enabled(&self) -> bool {
        self.control & 0x8000 != 0
    }

    pub fn timing(&self) -> u16 {
        (self.control >> 6) & 3
    }

    pub fn word_size(&self) -> bool {
        self.control & 0x20 != 0
    }

    pub fn repeat(&self) -> bool {
        self.control & 0x10 != 0
    }

    pub fn irq_enabled(&self) -> bool {
        self.control & 0x0300 != 0
    }

    pub fn dest_control(&self) -> u16 {
        self.control & 3
    }

    pub fn src_control(&self) -> u16 {
        (self.control >> 2) & 3
    }

    pub fn count_value(&self, index: usize) -> u32 {
        match self.count {
            0 if index == 3 => 0x1_0000,
            0 => 0x4000,
            n => n as u32,
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct Dma {
    pub channels: [DmaChannel; 4],
}

impl Dma {
    pub fn read16(&self, offset: u16) -> u16 {
        let (index, reg) = channel_reg(offset);
        let Some(ch) = self.channels.get(index) else {
            return 0;
        };
        match reg {
            0x0 => ch.src as u16,
            0x2 => (ch.src >> 16) as u16,
            0x4 => ch.dst as u16,
            0x6 => (ch.dst >> 16) as u16,
            0x8 => ch.count,
            0xA => ch.control,
            _ => 0,
        }
    }

    pub fn write16(&mut self, offset: u16, value: u16) {
        let (index, reg) = channel_reg(offset);
        let Some(ch) = self.channels.get_mut(index) else {
            return;
        };
        match reg {
            0x0 => ch.src = (ch.src & 0xFFFF_0000) | value as u32,
            0x2 => ch.src = (ch.src & 0x0000_FFFF) | ((value as u32) << 16),
            0x4 => ch.dst = (ch.dst & 0xFFFF_0000) | value as u32,
            0x6 => ch.dst = (ch.dst & 0x0000_FFFF) | ((value as u32) << 16),
            0x8 => ch.count = value,
            0xA => ch.control = value,
            _ => {}
        }
    }

    pub fn channel_count(offset: u16) -> Option<usize> {
        let (index, reg) = channel_reg(offset);
        (reg == 0xA && offset < 0xE0).then_some(index)
    }

    pub fn clear(&mut self) {
        self.channels = Default::default();
    }
}

fn channel_reg(offset: u16) -> (usize, u16) {
    if !(0xB0..0xE0).contains(&offset) {
        return (4, 0xFF);
    }
    let relative = offset - 0xB0;
    let index = (relative / 12) as usize;
    let reg = relative % 12;
    (index.min(3), reg)
}
