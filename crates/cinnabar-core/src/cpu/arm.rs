//! ARM (32-bit) instruction execution.

use super::*;
use crate::bus::Memory;

pub fn execute<M: Memory>(cpu: &mut Cpu, bus: &mut M, op: u32) {
    let cond = (op >> 28) & 0xF;
    if !cpu.condition_passed(cond) {
        return;
    }

    // BX is encoded in the miscellaneous data-processing space.
    if op & 0x0FFF_FFF0 == 0x012F_FF10 {
        let rm = (op & 0xF) as usize;
        let target = cpu.reg(rm);
        cpu.branch_exchange(target);
        cpu.add_cycles(2);
        return;
    }

    match (op >> 25) & 0x7 {
        0b000 => {
            if op & 0x0FC0_00F0 == 0x0000_0090 {
                multiply(cpu, op);
            } else if op & 0x0F80_00F0 == 0x0080_0090 {
                multiply_long(cpu, op);
            } else if op & 0x0FB0_0FF0 == 0x0100_0090 {
                swap(cpu, bus, op);
            } else if op & 0x0FBF_0FFF == 0x010F_0000 {
                mrs(cpu, op);
            } else if op & 0x0FB0_F000 == 0x0120_F000 {
                msr(cpu, op);
            } else if op & 0x0000_0090 == 0x0000_0090 {
                halfword_transfer(cpu, bus, op);
            } else {
                data_processing(cpu, bus, op);
            }
        }
        0b001 => data_processing(cpu, bus, op),
        0b010 | 0b011 => single_data_transfer(cpu, bus, op),
        0b100 => block_transfer(cpu, bus, op),
        0b101 => branch(cpu, op),
        0b111 => {
            // SWI. The bus decides whether to HLE the call or raise an
            // exception; Cinnabar's default bus HLEs the GBA BIOS.
            bus.swi(cpu, op & 0x00FF_FFFF);
            cpu.add_cycles(2);
        }
        _ => cpu.undefined(),
    }
}

fn data_processing<M: Memory>(cpu: &mut Cpu, _bus: &mut M, op: u32) {
    let immediate = (op >> 25) & 1 != 0;
    let opcode = (op >> 21) & 0xF;
    let s = (op >> 20) & 1 != 0;
    let rn = ((op >> 16) & 0xF) as usize;
    let rd = ((op >> 12) & 0xF) as usize;

    let (operand2, shifter_carry) = if immediate {
        Cpu::rotated_immediate(op)
    } else {
        let rm = (op & 0xF) as usize;
        let shift_type = (op >> 5) & 3;
        if op & 0x10 != 0 {
            // Register-specified shift.
            if op & 0x80 != 0 {
                cpu.undefined();
                return;
            }
            let rs = ((op >> 8) & 0xF) as usize;
            let amount = cpu.reg(rs) & 0xFF;
            cpu.shift(cpu.reg(rm), shift_type, amount, true)
        } else {
            let amount = (op >> 7) & 0x1F;
            cpu.shift(cpu.reg(rm), shift_type, amount, false)
        }
    };

    let operand1 = cpu.reg(rn);
    let test = matches!(opcode, 8..=11);
    let set_flags = s || test;

    let result = match opcode {
        0x0 => {
            // AND
            let result = operand1 & operand2;
            if set_flags {
                cpu.set_nz(result);
                cpu.set_flag(FLAG_C, shifter_carry);
            }
            result
        }
        0x1 => {
            // EOR
            let result = operand1 ^ operand2;
            if set_flags {
                cpu.set_nz(result);
                cpu.set_flag(FLAG_C, shifter_carry);
            }
            result
        }
        0x2 => cpu.subtract(operand1, operand2, false, set_flags),
        0x3 => cpu.subtract(operand2, operand1, false, set_flags),
        0x4 => cpu.add_with_carry(operand1, operand2, false, set_flags),
        0x5 => {
            let carry = cpu.flag_c();
            cpu.add_with_carry(operand1, operand2, carry, set_flags)
        }
        0x6 => {
            let borrow = !cpu.flag_c();
            cpu.subtract(operand1, operand2, borrow, set_flags)
        }
        0x7 => {
            let borrow = !cpu.flag_c();
            cpu.subtract(operand2, operand1, borrow, set_flags)
        }
        0x8 => {
            // TST
            let result = operand1 & operand2;
            cpu.set_nz(result);
            cpu.set_flag(FLAG_C, shifter_carry);
            result
        }
        0x9 => {
            // TEQ
            let result = operand1 ^ operand2;
            cpu.set_nz(result);
            cpu.set_flag(FLAG_C, shifter_carry);
            result
        }
        0xA => {
            // CMP
            cpu.subtract(operand1, operand2, false, true);
            0
        }
        0xB => {
            // CMN
            cpu.add_with_carry(operand1, operand2, false, true);
            0
        }
        0xC => {
            // ORR
            let result = operand1 | operand2;
            if set_flags {
                cpu.set_nz(result);
                cpu.set_flag(FLAG_C, shifter_carry);
            }
            result
        }
        0xD => {
            // MOV
            let result = operand2;
            if set_flags {
                cpu.set_nz(result);
                cpu.set_flag(FLAG_C, shifter_carry);
            }
            result
        }
        0xE => {
            // BIC
            let result = operand1 & !operand2;
            if set_flags {
                cpu.set_nz(result);
                cpu.set_flag(FLAG_C, shifter_carry);
            }
            result
        }
        0xF => {
            // MVN
            let result = !operand2;
            if set_flags {
                cpu.set_nz(result);
                cpu.set_flag(FLAG_C, shifter_carry);
            }
            result
        }
        _ => unreachable!(),
    };

    if !test {
        if rd == 15 && s {
            cpu.restore_cpsr_from_spsr();
            cpu.set_reg(15, result);
        } else {
            cpu.set_reg(rd, result);
        }
    }
}

fn multiply(cpu: &mut Cpu, op: u32) {
    let accumulate = (op >> 21) & 1 != 0;
    let s = (op >> 20) & 1 != 0;
    let rd = ((op >> 16) & 0xF) as usize;
    let rn = ((op >> 12) & 0xF) as usize;
    let rs = ((op >> 8) & 0xF) as usize;
    let rm = (op & 0xF) as usize;

    let a = cpu.reg(rm);
    let b = cpu.reg(rs);
    let acc = if accumulate { cpu.reg(rn) } else { 0 };
    let result = a.wrapping_mul(b).wrapping_add(acc);

    cpu.set_reg(rd, result);
    if s {
        cpu.set_nz(result);
    }
    cpu.add_cycles(3);
}

fn multiply_long(cpu: &mut Cpu, op: u32) {
    let s = (op >> 20) & 1 != 0;
    let signed = (op >> 22) & 1 != 0;
    let accumulate = (op >> 21) & 1 != 0;
    let rd_hi = ((op >> 16) & 0xF) as usize;
    let rd_lo = ((op >> 12) & 0xF) as usize;
    let rs = ((op >> 8) & 0xF) as usize;
    let rm = (op & 0xF) as usize;

    let a = cpu.reg(rm);
    let b = cpu.reg(rs);
    let acc = if accumulate {
        ((cpu.reg(rd_hi) as u64) << 32) | cpu.reg(rd_lo) as u64
    } else {
        0
    };

    let result = if signed {
        let a = a as i32 as i64;
        let b = b as i32 as i64;
        let acc = acc as i64;
        a.wrapping_mul(b).wrapping_add(acc) as u64
    } else {
        (a as u64)
            .wrapping_mul(b as u64)
            .wrapping_add(acc)
    };

    cpu.set_reg(rd_lo, result as u32);
    cpu.set_reg(rd_hi, (result >> 32) as u32);
    if s {
        cpu.set_nz64(result);
    }
    cpu.add_cycles(4);
}

fn swap<M: Memory>(cpu: &mut Cpu, bus: &mut M, op: u32) {
    let byte = (op >> 22) & 1 != 0;
    let rn = ((op >> 16) & 0xF) as usize;
    let rd = ((op >> 12) & 0xF) as usize;
    let rm = (op & 0xF) as usize;

    let addr = cpu.reg(rn);
    let value = cpu.reg(rm);
    let loaded = if byte {
        let old = bus.read8(addr) as u32;
        bus.write8(addr, value as u8);
        old
    } else {
        let old = bus.read32(addr);
        bus.write32(addr, value);
        old
    };

    cpu.set_reg(rd, loaded);
    cpu.add_cycles(3);
}

fn mrs(cpu: &mut Cpu, op: u32) {
    let spsr = (op >> 22) & 1 != 0;
    let rd = ((op >> 12) & 0xF) as usize;
    let value = if spsr && !matches!(cpu.mode(), Mode::User | Mode::System) {
        cpu.spsr()
    } else {
        cpu.cpsr
    };
    cpu.set_reg(rd, value);
}

fn msr(cpu: &mut Cpu, op: u32) {
    let spsr = (op >> 22) & 1 != 0;
    let mut mask = (op >> 16) & 0xF;
    let value = if op & (1 << 25) != 0 {
        Cpu::rotated_immediate(op).0
    } else {
        cpu.reg((op & 0xF) as usize)
    };

    if cpu.mode() == Mode::User {
        mask &= 0b1000;
    }

    let apply = |old: u32, value: u32, mask: u32| -> u32 {
        let mut out = old;
        if mask & 0b0001 != 0 {
            out = (out & !0x0000_00FF) | (value & 0x0000_00FF);
        }
        if mask & 0b0010 != 0 {
            out = (out & !0x0000_FF00) | (value & 0x0000_FF00);
        }
        if mask & 0b0100 != 0 {
            out = (out & !0x00FF_0000) | (value & 0x00FF_0000);
        }
        if mask & 0b1000 != 0 {
            out = (out & !0xFF00_0000) | (value & 0xFF00_0000);
        }
        out
    };

    if spsr {
        let new = apply(cpu.spsr(), value, mask);
        cpu.set_spsr(new);
    } else {
        let new = apply(cpu.cpsr, value, mask);
        cpu.set_cpsr(new);
    }
}

fn halfword_transfer<M: Memory>(cpu: &mut Cpu, bus: &mut M, op: u32) {
    let pre = (op >> 24) & 1 != 0;
    let up = (op >> 23) & 1 != 0;
    let immediate = (op >> 22) & 1 != 0;
    let writeback = (op >> 21) & 1 != 0;
    let load = (op >> 20) & 1 != 0;
    let rn = ((op >> 16) & 0xF) as usize;
    let rd = ((op >> 12) & 0xF) as usize;
    let signed = (op >> 6) & 1 != 0;
    let half = (op >> 5) & 1 != 0;

    let offset = if immediate {
        ((op >> 4) & 0xF0) | (op & 0xF)
    } else {
        cpu.reg((op & 0xF) as usize)
    };

    let base = cpu.reg(rn);
    let addr = if pre {
        if up {
            base.wrapping_add(offset)
        } else {
            base.wrapping_sub(offset)
        }
    } else {
        base
    };
    let post = if up {
        offset
    } else {
        offset.wrapping_neg()
    };

    if load {
        let value = match (signed, half) {
            (false, true) => bus.read16(addr) as u32,
            (true, false) => bus.read8(addr) as i8 as i32 as u32,
            (true, true) => bus.read16(addr) as i16 as i32 as u32,
            (false, false) => {
                cpu.undefined();
                return;
            }
        };
        cpu.set_reg(rd, value);
    } else if !signed && half {
        bus.write16(addr, cpu.reg(rd) as u16);
    } else {
        cpu.undefined();
        return;
    }

    if !pre || writeback {
        cpu.set_reg(rn, base.wrapping_add(post));
    }
    cpu.add_cycles(1);
}

fn single_data_transfer<M: Memory>(cpu: &mut Cpu, bus: &mut M, op: u32) {
    let register_offset = (op >> 25) & 1 != 0;
    let pre = (op >> 24) & 1 != 0;
    let up = (op >> 23) & 1 != 0;
    let byte = (op >> 22) & 1 != 0;
    let writeback = (op >> 21) & 1 != 0;
    let load = (op >> 20) & 1 != 0;
    let rn = ((op >> 16) & 0xF) as usize;
    let rd = ((op >> 12) & 0xF) as usize;

    let offset = if register_offset {
        if op & 0x10 != 0 {
            cpu.undefined();
            return;
        }
        let rm = (op & 0xF) as usize;
        let shift_type = (op >> 5) & 3;
        let amount = (op >> 7) & 0x1F;
        cpu.shift(cpu.reg(rm), shift_type, amount, false).0
    } else {
        op & 0xFFF
    };

    let base = cpu.reg(rn);
    let addr = if pre {
        if up {
            base.wrapping_add(offset)
        } else {
            base.wrapping_sub(offset)
        }
    } else {
        base
    };
    let post = if up {
        offset
    } else {
        offset.wrapping_neg()
    };

    if load {
        let value = if byte {
            bus.read8(addr) as u32
        } else {
            let value = bus.read32(addr);
            if addr & 3 != 0 {
                value.rotate_right((addr & 3) * 8)
            } else {
                value
            }
        };
        cpu.set_reg(rd, value);
    } else {
        let value = cpu.reg(rd);
        if byte {
            bus.write8(addr, value as u8);
        } else {
            bus.write32(addr, value);
        }
    }

    if !pre || writeback {
        cpu.set_reg(rn, base.wrapping_add(post));
    }
    cpu.add_cycles(1);
}

fn block_transfer<M: Memory>(cpu: &mut Cpu, bus: &mut M, op: u32) {
    let pre = (op >> 24) & 1 != 0;
    let up = (op >> 23) & 1 != 0;
    let user_bank = (op >> 22) & 1 != 0;
    let writeback = (op >> 21) & 1 != 0;
    let load = (op >> 20) & 1 != 0;
    let rn = ((op >> 16) & 0xF) as usize;
    let list = (op & 0xFFFF) as u16;
    let count = list.count_ones();

    let base = cpu.reg(rn);
    let start = match (pre, up) {
        (false, true) => base,
        (true, true) => base.wrapping_add(4),
        (false, false) => base.wrapping_sub(count.wrapping_mul(4)).wrapping_add(4),
        (true, false) => base.wrapping_sub(count.wrapping_mul(4)),
    };

    if list == 0 {
        // ARM7TDMI's documented empty-list quirk: transfer PC and move the
        // base by 0x40.
        if load {
            let value = bus.read32(base);
            cpu.set_reg(15, value);
        } else {
            bus.write32(base, cpu.reg(15));
        }
        if writeback {
            cpu.set_reg(rn, base.wrapping_add(0x40));
        }
        cpu.add_cycles(1);
        return;
    }

    let mut addr = start;
    let mut deferred_pc = None;

    for i in 0..16 {
        if list & (1 << i) == 0 {
            continue;
        }
        if load {
            let value = bus.read32(addr);
            if i == 15 && user_bank {
                deferred_pc = Some(value);
            } else if i == 13 || i == 14 {
                // Loading into SP/LR with the user-bank bit is rare; keeping
                // the current bank is the pragmatic choice.
                cpu.set_reg(i, value);
            } else {
                cpu.set_reg(i, value);
            }
        } else {
            bus.write32(addr, cpu.reg(i));
        }
        addr = addr.wrapping_add(4);
    }

    if let Some(pc) = deferred_pc {
        cpu.restore_cpsr_from_spsr();
        cpu.set_reg(15, pc);
    }

    if writeback && !(load && list & (1 << 15) != 0) {
        let new_base = if up {
            base.wrapping_add(count.wrapping_mul(4))
        } else {
            base.wrapping_sub(count.wrapping_mul(4))
        };
        cpu.set_reg(rn, new_base);
    }

    cpu.add_cycles(count);
}

fn branch(cpu: &mut Cpu, op: u32) {
    let link = (op >> 24) & 1 != 0;
    let offset = ((op << 8) as i32) >> 6;
    let target = cpu.reg(15).wrapping_add(offset as u32);

    if link {
        cpu.set_reg(14, cpu.r[15]);
    }
    cpu.set_reg(15, target);
    cpu.add_cycles(2);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cpu::tests_support::TestBus;

    fn run(cpu: &mut Cpu, bus: &mut TestBus, op: u32) {
        let pc = cpu.r[15];
        <TestBus as Memory>::write32(bus, pc, op);
        cpu.step(bus);
    }

    #[test]
    fn mov_immediate() {
        let mut cpu = Cpu::new();
        let mut bus = TestBus::default();
        cpu.r[15] = 0x0800_0000;
        run(&mut cpu, &mut bus, 0xE3A0_0001);
        assert_eq!(cpu.r[0], 1);
        assert_eq!(cpu.r[15], 0x0800_0004);
    }

    #[test]
    fn add_immediate() {
        let mut cpu = Cpu::new();
        let mut bus = TestBus::default();
        cpu.r[15] = 0x0800_0000;
        cpu.r[0] = 40;
        run(&mut cpu, &mut bus, 0xE280_1002);
        assert_eq!(cpu.r[1], 42);
    }

    #[test]
    fn cmp_sets_zero_flag() {
        let mut cpu = Cpu::new();
        let mut bus = TestBus::default();
        cpu.r[15] = 0x0800_0000;
        cpu.r[0] = 1;
        run(&mut cpu, &mut bus, 0xE350_0001);
        assert!(cpu.flag_z());
    }

    #[test]
    fn branch_to_next_instruction() {
        let mut cpu = Cpu::new();
        let mut bus = TestBus::default();
        cpu.r[15] = 0x0800_0000;
        run(&mut cpu, &mut bus, 0xEA00_0000);
        assert_eq!(cpu.r[15], 0x0800_0008);
    }

    #[test]
    fn store_and_load_word() {
        let mut cpu = Cpu::new();
        let mut bus = TestBus::default();
        cpu.r[15] = 0x0800_0000;
        cpu.r[0] = 0xDEAD_BEEF;
        cpu.r[1] = 0x0300_0000;
        run(&mut cpu, &mut bus, 0xE581_0000);
        assert_eq!(<TestBus as Memory>::read32(&mut bus, 0x0300_0000), 0xDEAD_BEEF);
        run(&mut cpu, &mut bus, 0xE591_2000);
        assert_eq!(cpu.r[2], 0xDEAD_BEEF);
    }

    #[test]
    fn block_store_and_load() {
        let mut cpu = Cpu::new();
        let mut bus = TestBus::default();
        cpu.r[15] = 0x0800_0000;
        cpu.r[0] = 0x0300_0000;
        cpu.r[1] = 0x1111_1111;
        cpu.r[2] = 0x2222_2222;
        run(&mut cpu, &mut bus, 0xE8A0_0006);
        assert_eq!(<TestBus as Memory>::read32(&mut bus, 0x0300_0000), 0x1111_1111);
        assert_eq!(<TestBus as Memory>::read32(&mut bus, 0x0300_0004), 0x2222_2222);
        cpu.r[0] = 0x0300_0000;
        run(&mut cpu, &mut bus, 0xE8B0_0018);
        assert_eq!(cpu.r[3], 0x1111_1111);
        assert_eq!(cpu.r[4], 0x2222_2222);
    }

    #[test]
    fn multiply() {
        let mut cpu = Cpu::new();
        let mut bus = TestBus::default();
        cpu.r[15] = 0x0800_0000;
        cpu.r[1] = 6;
        cpu.r[2] = 7;
        run(&mut cpu, &mut bus, 0xE000_0291);
        assert_eq!(cpu.r[0], 42);
    }

    #[test]
    fn bx_switches_to_thumb() {
        let mut cpu = Cpu::new();
        let mut bus = TestBus::default();
        cpu.r[15] = 0x0800_0000;
        cpu.r[0] = 0x0800_0101;
        run(&mut cpu, &mut bus, 0xE12F_FF10);
        assert!(cpu.thumb());
        assert_eq!(cpu.r[15], 0x0800_0100);
    }

    #[test]
    fn irq_enters_irq_mode() {
        let mut cpu = Cpu::new();
        cpu.r[15] = 0x0800_0000;
        cpu.cpsr &= !FLAG_I;
        cpu.irq();
        assert_eq!(cpu.mode(), Mode::Irq);
        assert_eq!(cpu.r[15], 0x18);
        assert_eq!(cpu.r[14], 0x0800_0004);
        assert_eq!(cpu.spsr() & 0x1F, Mode::System as u32);
    }
}
