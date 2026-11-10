//! ARM7TDMI interpreter.
//!
//! The CPU is a straightforward fetch-decode-execute core. `r[15]` always
//! holds the address of the *next* instruction to fetch; reads of r15 apply
//! the architectural pipeline bias (+4 in ARM, +2 in Thumb). This keeps the
//! implementation honest without modelling the prefetch buffer.

mod arm;
mod thumb;

use crate::bus::Memory;

pub const FLAG_N: u32 = 1 << 31;
pub const FLAG_Z: u32 = 1 << 30;
pub const FLAG_C: u32 = 1 << 29;
pub const FLAG_V: u32 = 1 << 28;
pub const FLAG_I: u32 = 1 << 7;
pub const FLAG_F: u32 = 1 << 6;
pub const FLAG_T: u32 = 1 << 5;

/// CPU modes, with their CPSR mode-bit encodings.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    User = 0x10,
    Fiq = 0x11,
    Irq = 0x12,
    Supervisor = 0x13,
    Abort = 0x17,
    Undefined = 0x1B,
    System = 0x1F,
}

impl Mode {
    pub fn from_bits(bits: u32) -> Option<Self> {
        Some(match bits & 0x1F {
            0x10 => Self::User,
            0x11 => Self::Fiq,
            0x12 => Self::Irq,
            0x13 => Self::Supervisor,
            0x17 => Self::Abort,
            0x1B => Self::Undefined,
            0x1F => Self::System,
            _ => return None,
        })
    }

    /// User and System share the same banked SP/LR, so they share an index.
    const fn bank(self) -> usize {
        match self {
            Self::User | Self::System => 0,
            Self::Supervisor => 1,
            Self::Abort => 2,
            Self::Undefined => 3,
            Self::Irq => 4,
            Self::Fiq => 5,
        }
    }
}

#[derive(Clone, Debug, Default)]
struct Banks {
    r13: [u32; 6],
    r14: [u32; 6],
    spsr: [u32; 6],
    fiq_r8_12: [u32; 5],
}

/// ARM7TDMI register state.
#[derive(Clone, Debug)]
pub struct Cpu {
    pub(crate) r: [u32; 16],
    pub(crate) cpsr: u32,
    banks: Banks,
    cycles: u32,
}

impl Default for Cpu {
    fn default() -> Self {
        Self::new()
    }
}

impl Cpu {
    pub fn new() -> Self {
        Self {
            r: [0; 16],
            cpsr: Mode::System as u32,
            banks: Banks::default(),
            cycles: 0,
        }
    }

    /// Reset the CPU to the state the GBA BIOS leaves it in before jumping to
    /// the cartridge: ARM state, System mode, IRQ/FIQ enabled, SP at the top
    /// of IWRAM.
    pub fn reset_to_cart(&mut self) {
        self.r = [0; 16];
        self.cpsr = Mode::System as u32;
        self.banks = Banks::default();

        // The BIOS leaves these stacks behind for the cartridge to use.
        self.banks.r13[Mode::User.bank()] = 0x0300_7F00;
        self.banks.r13[Mode::Supervisor.bank()] = 0x0300_7FE0;
        self.banks.r13[Mode::Abort.bank()] = 0x0300_7F00;
        self.banks.r13[Mode::Undefined.bank()] = 0x0300_7F00;
        self.banks.r13[Mode::Irq.bank()] = 0x0300_7FA0;
        self.banks.r13[Mode::Fiq.bank()] = 0x0300_7F00;

        self.r[13] = 0x0300_7F00;
        self.r[15] = 0x0800_0000;
        self.cycles = 0;
    }

    #[inline]
    pub fn pc(&self) -> u32 {
        self.r[15]
    }

    #[inline]
    pub fn cpsr(&self) -> u32 {
        self.cpsr
    }

    #[inline]
    pub fn regs(&self) -> &[u32; 16] {
        &self.r
    }

    #[inline]
    pub fn mode(&self) -> Mode {
        Mode::from_bits(self.cpsr).unwrap_or(Mode::System)
    }

    #[inline]
    pub fn thumb(&self) -> bool {
        self.cpsr & FLAG_T != 0
    }

    #[inline]
    pub fn irq_enabled(&self) -> bool {
        self.cpsr & FLAG_I == 0
    }

    #[inline]
    pub fn fiq_enabled(&self) -> bool {
        self.cpsr & FLAG_F == 0
    }

    #[inline]
    pub(crate) fn flag(&self, flag: u32) -> bool {
        self.cpsr & flag != 0
    }

    #[inline]
    pub(crate) fn flag_n(&self) -> bool {
        self.flag(FLAG_N)
    }
    #[inline]
    pub(crate) fn flag_z(&self) -> bool {
        self.flag(FLAG_Z)
    }
    #[inline]
    pub(crate) fn flag_c(&self) -> bool {
        self.flag(FLAG_C)
    }
    #[inline]
    pub(crate) fn flag_v(&self) -> bool {
        self.flag(FLAG_V)
    }

    #[inline]
    pub(crate) fn set_flag(&mut self, flag: u32, value: bool) {
        if value {
            self.cpsr |= flag;
        } else {
            self.cpsr &= !flag;
        }
    }

    #[inline]
    pub(crate) fn set_nz(&mut self, value: u32) {
        self.set_flag(FLAG_N, value & 0x8000_0000 != 0);
        self.set_flag(FLAG_Z, value == 0);
    }

    #[inline]
    pub(crate) fn set_nz64(&mut self, value: u64) {
        self.set_flag(FLAG_N, value & (1 << 63) != 0);
        self.set_flag(FLAG_Z, value == 0);
    }

    /// Evaluate an ARM condition code.
    pub(crate) fn condition_passed(&self, cond: u32) -> bool {
        let n = self.flag_n();
        let z = self.flag_z();
        let c = self.flag_c();
        let v = self.flag_v();
        match cond {
            0x0 => z,
            0x1 => !z,
            0x2 => c,
            0x3 => !c,
            0x4 => n,
            0x5 => !n,
            0x6 => v,
            0x7 => !v,
            0x8 => c && !z,
            0x9 => !c || z,
            0xA => n == v,
            0xB => n != v,
            0xC => !z && n == v,
            0xD => z || n != v,
            0xE => true,
            _ => false,
        }
    }

    /// Architectural read of a register. Reading r15 applies the pipeline bias.
    #[inline]
    pub(crate) fn reg(&self, index: usize) -> u32 {
        if index == 15 {
            self.r[15]
                .wrapping_add(if self.thumb() { 2 } else { 4 })
        } else {
            self.r[index]
        }
    }

    /// Architectural write of a register. Writing r15 cannot change instruction
    /// state; only BX/BLX (and exception return) do that on ARMv4T.
    #[inline]
    pub(crate) fn set_reg(&mut self, index: usize, value: u32) {
        if index == 15 {
            self.branch(value);
        } else {
            self.r[index] = value;
        }
    }

    #[inline]
    pub(crate) fn branch(&mut self, value: u32) {
        self.r[15] = if self.thumb() { value & !1 } else { value & !3 };
    }

    #[inline]
    pub(crate) fn branch_exchange(&mut self, value: u32) {
        let thumb = value & 1 != 0;
        self.set_thumb(thumb);
        self.r[15] = if thumb { value & !1 } else { value & !3 };
    }

    #[inline]
    pub(crate) fn set_thumb(&mut self, thumb: bool) {
        self.set_flag(FLAG_T, thumb);
    }

    #[inline]
    pub(crate) fn add_cycles(&mut self, cycles: u32) {
        self.cycles += cycles;
    }

    /// Barrel shifter. Returns `(result, carry_out)`. When `by_register` is
    /// true and the shift amount is zero, the carry flag is preserved, as the
    /// ARM ARM requires.
    pub(crate) fn shift(
        &self,
        value: u32,
        shift_type: u32,
        amount: u32,
        by_register: bool,
    ) -> (u32, bool) {
        let c = self.flag_c();
        if by_register && amount == 0 {
            return (value, c);
        }
        match shift_type {
            0 => {
                // LSL
                let amount = amount & 0xFF;
                match amount {
                    0 => (value, c),
                    1..=31 => (value << amount, (value >> (32 - amount)) & 1 != 0),
                    32 => (0, value & 1 != 0),
                    _ => (0, false),
                }
            }
            1 => {
                // LSR
                let amount = if amount == 0 { 32 } else { amount & 0xFF };
                match amount {
                    1..=31 => (value >> amount, (value >> (amount - 1)) & 1 != 0),
                    32 => (0, value & 0x8000_0000 != 0),
                    _ => (0, false),
                }
            }
            2 => {
                // ASR
                let amount = if amount == 0 { 32 } else { amount & 0xFF };
                match amount {
                    1..=31 => (
                        ((value as i32) >> amount) as u32,
                        (value >> (amount - 1)) & 1 != 0,
                    ),
                    32 => (
                        if value & 0x8000_0000 != 0 {
                            0xFFFF_FFFF
                        } else {
                            0
                        },
                        value & 0x8000_0000 != 0,
                    ),
                    _ => (
                        if value & 0x8000_0000 != 0 {
                            0xFFFF_FFFF
                        } else {
                            0
                        },
                        value & 0x8000_0000 != 0,
                    ),
                }
            }
            _ => {
                // ROR / RRX
                let amount = amount & 0xFF;
                if amount == 0 {
                    if by_register {
                        (value, c)
                    } else {
                        // RRX: carry into bit 31
                        (
                            (value >> 1) | ((c as u32) << 31),
                            value & 1 != 0,
                        )
                    }
                } else {
                    let amount = amount % 32;
                    if amount == 0 {
                        (value, value & 0x8000_0000 != 0)
                    } else {
                        (
                            value.rotate_right(amount),
                            (value >> (amount - 1)) & 1 != 0,
                        )
                    }
                }
            }
        }
    }

    /// Add with carry, updating flags only when `set_flags` is true.
    /// Returns the result.
    pub(crate) fn add_with_carry(
        &mut self,
        a: u32,
        b: u32,
        carry: bool,
        set_flags: bool,
    ) -> u32 {
        let sum = a as u64 + b as u64 + carry as u64;
        let result = sum as u32;
        if set_flags {
            let carry_out = sum >> 32 != 0;
            let overflow = ((a ^ result) & (b ^ result)) & 0x8000_0000 != 0;
            self.set_nz(result);
            self.set_flag(FLAG_C, carry_out);
            self.set_flag(FLAG_V, overflow);
        }
        result
    }

    /// Subtract with borrow (`borrow` is 1 when a borrow is required, i.e.
    /// when carry is clear). Returns the result.
    pub(crate) fn subtract(
        &mut self,
        a: u32,
        b: u32,
        borrow: bool,
        set_flags: bool,
    ) -> u32 {
        let sub = (a as u64)
            .wrapping_sub(b as u64)
            .wrapping_sub(borrow as u64);
        let result = sub as u32;
        if set_flags {
            let carry_out = (a as u64) >= (b as u64 + borrow as u64);
            let overflow = ((a ^ b) & (a ^ result)) & 0x8000_0000 != 0;
            self.set_nz(result);
            self.set_flag(FLAG_C, carry_out);
            self.set_flag(FLAG_V, overflow);
        }
        result
    }

    /// Rotated 8-bit immediate used by data-processing instructions.
    #[inline]
    pub(crate) fn rotated_immediate(op: u32) -> (u32, bool) {
        let imm = op & 0xFF;
        let rotate = ((op >> 8) & 0xF) * 2;
        let value = imm.rotate_right(rotate);
        (value, rotate != 0 && value & 0x8000_0000 != 0)
    }

    /// Fetch, decode, and execute one instruction. Returns the number of
    /// cycles consumed.
    pub fn step<M: Memory>(&mut self, bus: &mut M) -> u32 {
        self.cycles = 1;
        if self.thumb() {
            let op = bus.read16(self.r[15]) as u32;
            self.r[15] = self.r[15].wrapping_add(2);
            thumb::execute(self, bus, op);
        } else {
            let op = bus.read32(self.r[15]);
            self.r[15] = self.r[15].wrapping_add(4);
            arm::execute(self, bus, op);
        }
        self.cycles.max(1)
    }

    /// Enter an exception. `lr_offset` is added to the architectural next PC
    /// to form the return address (4 for ARM IRQ, 2 for Thumb IRQ, 0 for SWI).
    pub(crate) fn enter_exception(&mut self, vector: u32, mode: Mode, lr_offset: u32) {
        let old_cpsr = self.cpsr;
        let return_addr = self.r[15].wrapping_add(lr_offset);
        self.switch_mode(mode);
        self.banks.spsr[mode.bank()] = old_cpsr;
        self.cpsr = (self.cpsr & !0x1F) | mode as u32 | FLAG_I;
        if mode == Mode::Fiq {
            self.cpsr |= FLAG_F;
        }
        self.set_thumb(false);
        self.r[14] = return_addr;
        self.r[15] = vector;
        self.cycles += 2;
    }

    pub fn irq(&mut self) {
        if self.irq_enabled() {
            let offset = if self.thumb() { 2 } else { 4 };
            self.enter_exception(0x18, Mode::Irq, offset);
        }
    }

    pub fn fiq(&mut self) {
        if self.fiq_enabled() {
            let offset = if self.thumb() { 2 } else { 4 };
            self.enter_exception(0x1C, Mode::Fiq, offset);
        }
    }

    pub fn undefined(&mut self) {
        self.enter_exception(0x04, Mode::Undefined, 0);
    }

    pub(crate) fn switch_mode(&mut self, new: Mode) {
        let old = self.mode();
        if old == new {
            return;
        }
        self.banks.r13[old.bank()] = self.r[13];
        self.banks.r14[old.bank()] = self.r[14];
        if old == Mode::Fiq {
            self.banks.fiq_r8_12.copy_from_slice(&self.r[8..13]);
        }
        if new == Mode::Fiq {
            self.r[8..13].copy_from_slice(&self.banks.fiq_r8_12);
        }
        self.r[13] = self.banks.r13[new.bank()];
        self.r[14] = self.banks.r14[new.bank()];
    }

    pub(crate) fn set_cpsr(&mut self, value: u32) {
        let old = self.mode();
        let new = Mode::from_bits(value).unwrap_or(old);
        if new != old {
            self.switch_mode(new);
        }
        self.cpsr = (value & !0x1F) | new as u32;
    }

    pub(crate) fn spsr(&self) -> u32 {
        self.banks.spsr[self.mode().bank()]
    }

    pub(crate) fn set_spsr(&mut self, value: u32) {
        self.banks.spsr[self.mode().bank()] = value;
    }

    pub(crate) fn restore_cpsr_from_spsr(&mut self) {
        let spsr = self.spsr();
        self.set_cpsr(spsr);
    }
}

#[cfg(test)]
pub(crate) mod tests_support {
    use crate::bus::Memory;

    /// A flat 64 KiB test bus. Addresses are masked, so the CPU can execute
    /// from 0x0800_0000 while the backing array stays small.
    pub struct TestBus {
        pub mem: Vec<u8>,
    }

    impl Default for TestBus {
        fn default() -> Self {
            Self::new()
        }
    }

    impl TestBus {
        pub fn new() -> Self {
            Self {
                mem: vec![0; 0x2_0000],
            }
        }

        fn index(addr: u32) -> usize {
            if (0x0800_0000..0x0801_0000).contains(&addr) {
                (addr - 0x0800_0000) as usize
            } else if (0x0300_0000..0x0301_0000).contains(&addr) {
                0x1_0000 + (addr - 0x0300_0000) as usize
            } else {
                (addr & 0xFFFF) as usize
            }
        }
    }

    impl Memory for TestBus {
        fn read8(&mut self, addr: u32) -> u8 {
            self.mem[Self::index(addr)]
        }

        fn read16(&mut self, addr: u32) -> u16 {
            let a = Self::index(addr);
            u16::from_le_bytes([self.mem[a], self.mem[a + 1]])
        }

        fn read32(&mut self, addr: u32) -> u32 {
            let a = Self::index(addr);
            u32::from_le_bytes([
                self.mem[a],
                self.mem[a + 1],
                self.mem[a + 2],
                self.mem[a + 3],
            ])
        }

        fn write8(&mut self, addr: u32, value: u8) {
            self.mem[Self::index(addr)] = value;
        }

        fn write16(&mut self, addr: u32, value: u16) {
            let a = Self::index(addr);
            self.mem[a..a + 2].copy_from_slice(&value.to_le_bytes());
        }

        fn write32(&mut self, addr: u32, value: u32) {
            let a = Self::index(addr);
            self.mem[a..a + 4].copy_from_slice(&value.to_le_bytes());
        }
    }

    impl TestBus {
        pub fn write16(&mut self, addr: u32, value: u16) {
            <Self as Memory>::write16(self, addr, value);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flags_and_conditions() {
        let mut cpu = Cpu::new();
        cpu.set_nz(0);
        assert!(cpu.flag_z());
        assert!(cpu.condition_passed(0x0));
        assert!(!cpu.condition_passed(0x1));

        cpu.set_nz(0x8000_0000);
        assert!(cpu.flag_n());
        assert!(!cpu.flag_z());
        assert!(cpu.condition_passed(0x4));
        assert!(!cpu.condition_passed(0x5));
    }

    #[test]
    fn shift_carry_examples() {
        let cpu = Cpu::new();
        assert_eq!(cpu.shift(0x8000_0000, 0, 1, false), (0, true));
        assert_eq!(cpu.shift(1, 0, 32, false), (0, true));
        assert_eq!(cpu.shift(0x8000_0000, 2, 1, false), (0xC000_0000, false));
        assert_eq!(cpu.shift(1, 3, 0, false), (0, true));
    }

    #[test]
    fn add_subtract_flags() {
        let mut cpu = Cpu::new();
        assert_eq!(cpu.add_with_carry(0xFFFF_FFFF, 1, false, true), 0);
        assert!(cpu.flag_z());
        assert!(cpu.flag_c());
        assert!(!cpu.flag_v());

        assert_eq!(cpu.add_with_carry(0x7FFF_FFFF, 1, false, true), 0x8000_0000);
        assert!(cpu.flag_n());
        assert!(cpu.flag_v());
        assert!(!cpu.flag_c());

        assert_eq!(cpu.subtract(0, 1, false, true), 0xFFFF_FFFF);
        assert!(cpu.flag_n());
        assert!(!cpu.flag_c());
        assert!(!cpu.flag_v());
    }
}
