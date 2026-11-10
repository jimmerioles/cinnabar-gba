//! Four hardware timers.

use crate::irq::{self, Irq};

#[derive(Clone, Debug, Default)]
pub struct Timer {
    pub reload: u16,
    pub counter: u16,
    pub control: u16,
    accumulator: u32,
}

impl Timer {
    pub fn enabled(&self) -> bool {
        self.control & 0x80 != 0
    }

    pub fn cascade(&self) -> bool {
        self.control & 0x04 != 0
    }

    pub fn irq_enabled(&self) -> bool {
        self.control & 0x40 != 0
    }

    pub fn prescaler(&self) -> u32 {
        match self.control & 3 {
            0 => 1,
            1 => 64,
            2 => 256,
            _ => 1024,
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct Timers {
    pub timers: [Timer; 4],
}

impl Timers {
    pub fn step(&mut self, cycles: u32, irq: &mut Irq) {
        for i in 0..4 {
            if !self.timers[i].enabled() || self.timers[i].cascade() {
                continue;
            }
            let mut ticks = cycles;
            let prescaler = self.timers[i].prescaler();
            self.timers[i].accumulator += ticks % prescaler;
            ticks /= prescaler;
            if self.timers[i].accumulator >= prescaler {
                self.timers[i].accumulator -= prescaler;
                ticks += 1;
            }
            let overflows = self.advance(i, ticks);
            for _ in 0..overflows {
                self.cascade_from(i, irq);
            }
        }
    }

    /// Advance a timer by `ticks`; returns the number of overflows.
    fn advance(&mut self, index: usize, ticks: u32) -> u32 {
        if ticks == 0 {
            return 0;
        }
        let timer = &mut self.timers[index];
        let mut remaining = ticks;
        let mut overflows = 0;
        while remaining > 0 {
            let to_overflow = 0x1_0000u32 - timer.counter as u32;
            if remaining >= to_overflow {
                remaining -= to_overflow;
                timer.counter = timer.reload;
                overflows += 1;
            } else {
                timer.counter = timer.counter.wrapping_add(remaining as u16);
                remaining = 0;
            }
        }
        overflows
    }

    fn cascade_from(&mut self, index: usize, irq: &mut Irq) {
        if index < 3 {
            let next = index + 1;
            if self.timers[next].enabled() && self.timers[next].cascade() {
                let overflows = self.advance(next, 1);
                if overflows > 0 {
                    self.cascade_from(next, irq);
                }
            }
        }
        self.on_overflow(index, irq);
    }

    fn on_overflow(&mut self, index: usize, irq: &mut Irq) {
        if self.timers[index].irq_enabled() {
            irq.request(irq::bits::TIMER0 << index);
        }
    }

    pub fn read_reload(&self, index: usize) -> u16 {
        self.timers[index].reload
    }

    pub fn read_counter(&self, index: usize) -> u16 {
        self.timers[index].counter
    }

    pub fn read_control(&self, index: usize) -> u16 {
        self.timers[index].control
    }

    pub fn write_reload(&mut self, index: usize, value: u16) {
        let timer = &mut self.timers[index];
        timer.reload = value;
        if !timer.enabled() {
            timer.counter = value;
        }
    }

    pub fn write_control(&mut self, index: usize, value: u16) {
        let timer = &mut self.timers[index];
        let was_enabled = timer.enabled();
        timer.control = value & 0x00C7;
        let is_enabled = timer.enabled();
        if !was_enabled && is_enabled {
            timer.counter = timer.reload;
            timer.accumulator = 0;
        }
    }
}
