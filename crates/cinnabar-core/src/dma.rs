//! Direct Memory Access channels.

#[derive(Clone, Debug, Default)]
pub struct DmaChannel {
    pub src: u32,
    pub dst: u32,
    pub count: u16,
    pub control: u16,
    pub(crate) src_base: u32,
    pub(crate) dst_base: u32,
    pub(crate) count_base: u16,
}

impl DmaChannel {
    pub fn enabled(&self) -> bool {
        self.control & 0x8000 != 0
    }

    pub fn timing(&self) -> u16 {
        (self.control >> 12) & 3
    }

    pub fn word_size(&self) -> bool {
        self.control & (1 << 10) != 0
    }

    pub fn repeat(&self) -> bool {
        self.control & (1 << 9) != 0
    }

    pub fn irq_enabled(&self) -> bool {
        self.control & (1 << 14) != 0
    }

    pub fn dest_control(&self) -> u16 {
        (self.control >> 5) & 3
    }

    pub fn src_control(&self) -> u16 {
        (self.control >> 7) & 3
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
            0x0 => {
                ch.src = (ch.src & 0xFFFF_0000) | value as u32;
                if !ch.enabled() {
                    ch.src_base = ch.src;
                }
            }
            0x2 => {
                ch.src = (ch.src & 0x0000_FFFF) | ((value as u32) << 16);
                if !ch.enabled() {
                    ch.src_base = ch.src;
                }
            }
            0x4 => {
                ch.dst = (ch.dst & 0xFFFF_0000) | value as u32;
                if !ch.enabled() {
                    ch.dst_base = ch.dst;
                }
            }
            0x6 => {
                ch.dst = (ch.dst & 0x0000_FFFF) | ((value as u32) << 16);
                if !ch.enabled() {
                    ch.dst_base = ch.dst;
                }
            }
            0x8 => {
                ch.count = value;
                if !ch.enabled() {
                    ch.count_base = value;
                }
            }
            0xA => {
                let was_enabled = ch.enabled();
                ch.control = value;
                if !was_enabled && ch.enabled() && ch.repeat() {
                    // A fresh repeat transfer reloads the address/count
                    // latches. Sound DMA in particular relies on this when
                    // its VBlank handler restarts the channel.
                    ch.src = ch.src_base;
                    ch.dst = ch.dst_base;
                    ch.count = ch.count_base;
                }
            }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repeat_reloads_source_when_reenabled() {
        let mut dma = Dma::default();
        dma.write16(0x0BC, 0x0008);
        dma.write16(0x0BE, 0x0200);
        dma.write16(0x0C0, 0x00A0);
        dma.write16(0x0C2, 0x0400);
        dma.write16(0x0C4, 0x0004);
        dma.write16(0x0C6, 0xB600);

        let channel = &dma.channels[1];
        assert_eq!(channel.src, 0x0200_0008);
        assert_eq!(channel.dst, 0x0400_00A0);
        assert_eq!(channel.count, 4);

        // Simulate the source advancing during a sound-DMA refill.
        dma.channels[1].src += 0x20;
        dma.write16(0x0C6, 0x0440);
        assert_eq!(dma.channels[1].src, 0x0200_0028);

        // The VBlank handler restarts the channel; it must wrap to the
        // latched source rather than running linearly through EWRAM.
        dma.write16(0x0C6, 0xB600);
        assert_eq!(dma.channels[1].src, 0x0200_0008);
        assert_eq!(dma.channels[1].count, 4);
    }

    #[test]
    fn one_shot_channels_keep_their_advanced_source() {
        let mut dma = Dma::default();
        dma.write16(0x0BC, 0x0008);
        dma.write16(0x0BE, 0x0200);
        dma.write16(0x0C4, 0x0004);
        dma.write16(0x0C6, 0x8400);

        dma.channels[1].src += 0x20;
        dma.write16(0x0C6, 0x0400);
        dma.write16(0x0C6, 0x8400);

        assert_eq!(dma.channels[1].src, 0x0200_0028);
    }
}
