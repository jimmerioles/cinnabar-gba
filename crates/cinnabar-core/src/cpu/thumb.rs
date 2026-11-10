//! Thumb (16-bit) instruction execution.

use super::*;
use crate::bus::Memory;

pub fn execute<M: Memory>(cpu: &mut Cpu, bus: &mut M, op: u32) {
    match op >> 12 {
        0x0 | 0x1 => {
            if op & 0xF800 == 0x1800 {
                add_subtract(cpu, op);
            } else {
                move_shifted(cpu, op);
            }
        }
        0x2 | 0x3 => immediate_ops(cpu, op),
        0x4 => match op & 0xFC00 {
            0x4000 => alu(cpu, op),
            0x4400 => hi_register(cpu, op),
            _ => {
                if op & 0xF800 == 0x4800 {
                    pc_relative_load(cpu, bus, op);
                } else {
                    cpu.undefined();
                }
            }
        },
        0x5 => {
            if op & 0x0200 != 0 {
                load_store_signed(cpu, bus, op);
            } else {
                load_store_register(cpu, bus, op);
            }
        }
        0x6 | 0x7 => load_store_immediate(cpu, bus, op),
        0x8 => load_store_halfword_immediate(cpu, bus, op),
        0x9 => sp_relative(cpu, bus, op),
        0xA => load_address(cpu, op),
        0xB => {
            if op & 0xFF00 == 0xB000 {
                add_sp(cpu, op);
            } else if op & 0xF600 == 0xB400 {
                push_pop(cpu, bus, op);
            } else {
                cpu.undefined();
            }
        }
        0xC => multiple(cpu, bus, op),
        0xD => {
            if op & 0xFF00 == 0xDF00 {
                bus.swi(cpu, op & 0xFF);
                cpu.add_cycles(2);
            } else {
                conditional_branch(cpu, op);
            }
        }
        0xE => unconditional_branch(cpu, op),
        0xF => long_branch(cpu, op),
        _ => unreachable!(),
    }
}

fn move_shifted(cpu: &mut Cpu, op: u32) {
    let kind = (op >> 11) & 3;
    let amount = (op >> 6) & 0x1F;
    let rs = ((op >> 3) & 7) as usize;
    let rd = (op & 7) as usize;

    let (result, carry) = match kind {
        0 => cpu.shift(cpu.reg(rs), 0, amount, false),
        1 => cpu.shift(cpu.reg(rs), 1, amount, false),
        2 => cpu.shift(cpu.reg(rs), 2, amount, false),
        _ => {
            cpu.undefined();
            return;
        }
    };

    cpu.set_reg(rd, result);
    cpu.set_nz(result);
    cpu.set_flag(FLAG_C, carry);
}

fn add_subtract(cpu: &mut Cpu, op: u32) {
    let immediate = (op >> 10) & 1 != 0;
    let subtract = (op >> 9) & 1 != 0;
    let operand = (op >> 6) & 7;
    let rs = ((op >> 3) & 7) as usize;
    let rd = (op & 7) as usize;

    let rhs = if immediate {
        operand
    } else {
        cpu.reg(operand as usize)
    };
    let lhs = cpu.reg(rs);

    let result = if subtract {
        cpu.subtract(lhs, rhs, false, true)
    } else {
        cpu.add_with_carry(lhs, rhs, false, true)
    };
    cpu.set_reg(rd, result);
}

fn immediate_ops(cpu: &mut Cpu, op: u32) {
    let kind = (op >> 11) & 3;
    let rd = ((op >> 8) & 7) as usize;
    let imm = op & 0xFF;

    match kind {
        0 => {
            // MOV
            cpu.set_reg(rd, imm);
            cpu.set_nz(imm);
        }
        1 => {
            // CMP
            cpu.subtract(cpu.reg(rd), imm, false, true);
        }
        2 => {
            // ADD
            let result = cpu.add_with_carry(cpu.reg(rd), imm, false, true);
            cpu.set_reg(rd, result);
        }
        3 => {
            // SUB
            let result = cpu.subtract(cpu.reg(rd), imm, false, true);
            cpu.set_reg(rd, result);
        }
        _ => unreachable!(),
    }
}

fn alu(cpu: &mut Cpu, op: u32) {
    let kind = (op >> 6) & 0xF;
    let rs = ((op >> 3) & 7) as usize;
    let rd = (op & 7) as usize;
    let a = cpu.reg(rd);
    let b = cpu.reg(rs);

    match kind {
        0x0 => {
            let result = a & b;
            cpu.set_reg(rd, result);
            cpu.set_nz(result);
        }
        0x1 => {
            let result = a ^ b;
            cpu.set_reg(rd, result);
            cpu.set_nz(result);
        }
        0x2 => {
            let (result, carry) = cpu.shift(a, 0, b & 0xFF, true);
            cpu.set_reg(rd, result);
            cpu.set_nz(result);
            cpu.set_flag(FLAG_C, carry);
        }
        0x3 => {
            let (result, carry) = cpu.shift(a, 1, b & 0xFF, true);
            cpu.set_reg(rd, result);
            cpu.set_nz(result);
            cpu.set_flag(FLAG_C, carry);
        }
        0x4 => {
            let (result, carry) = cpu.shift(a, 2, b & 0xFF, true);
            cpu.set_reg(rd, result);
            cpu.set_nz(result);
            cpu.set_flag(FLAG_C, carry);
        }
        0x5 => {
            let carry = cpu.flag_c();
            let result = cpu.add_with_carry(a, b, carry, true);
            cpu.set_reg(rd, result);
        }
        0x6 => {
            let borrow = !cpu.flag_c();
            let result = cpu.subtract(a, b, borrow, true);
            cpu.set_reg(rd, result);
        }
        0x7 => {
            let (result, carry) = cpu.shift(a, 3, b & 0xFF, true);
            cpu.set_reg(rd, result);
            cpu.set_nz(result);
            cpu.set_flag(FLAG_C, carry);
        }
        0x8 => {
            let result = a & b;
            cpu.set_nz(result);
        }
        0x9 => {
            let result = cpu.subtract(0, b, false, true);
            cpu.set_reg(rd, result);
        }
        0xA => {
            cpu.subtract(a, b, false, true);
        }
        0xB => {
            cpu.add_with_carry(a, b, false, true);
        }
        0xC => {
            let result = a | b;
            cpu.set_reg(rd, result);
            cpu.set_nz(result);
        }
        0xD => {
            let result = a.wrapping_mul(b);
            cpu.set_reg(rd, result);
            cpu.set_nz(result);
            cpu.add_cycles(2);
        }
        0xE => {
            let result = a & !b;
            cpu.set_reg(rd, result);
            cpu.set_nz(result);
        }
        0xF => {
            let result = !b;
            cpu.set_reg(rd, result);
            cpu.set_nz(result);
        }
        _ => unreachable!(),
    }
}

fn hi_register(cpu: &mut Cpu, op: u32) {
    let kind = (op >> 8) & 3;
    let h1 = (op >> 7) & 1;
    let h2 = (op >> 6) & 1;
    let rs = (((op >> 3) & 7) | (h2 << 3)) as usize;
    let rd = ((op & 7) | (h1 << 3)) as usize;

    match kind {
        0 => {
            let result = cpu.reg(rd).wrapping_add(cpu.reg(rs));
            cpu.set_reg(rd, result);
            if rd == 15 {
                cpu.add_cycles(2);
            }
        }
        1 => {
            cpu.subtract(cpu.reg(rd), cpu.reg(rs), false, true);
        }
        2 => {
            let value = cpu.reg(rs);
            cpu.set_reg(rd, value);
            if rd == 15 {
                cpu.add_cycles(2);
            }
        }
        3 => {
            let target = cpu.reg(rs);
            cpu.branch_exchange(target);
            cpu.add_cycles(2);
        }
        _ => unreachable!(),
    }
}

fn pc_relative_load<M: Memory>(cpu: &mut Cpu, bus: &mut M, op: u32) {
    let rd = ((op >> 8) & 7) as usize;
    let imm = (op & 0xFF) * 4;
    let addr = (cpu.reg(15) & !2).wrapping_add(imm);
    cpu.set_reg(rd, read_word(bus, addr));
    cpu.add_cycles(1);
}

fn load_store_register<M: Memory>(cpu: &mut Cpu, bus: &mut M, op: u32) {
    let load = (op >> 11) & 1 != 0;
    let byte = (op >> 10) & 1 != 0;
    let ro = ((op >> 6) & 7) as usize;
    let rb = ((op >> 3) & 7) as usize;
    let rd = (op & 7) as usize;
    let addr = cpu.reg(rb).wrapping_add(cpu.reg(ro));

    if load {
        let value = if byte {
            bus.read8(addr) as u32
        } else {
            read_word(bus, addr)
        };
        cpu.set_reg(rd, value);
    } else if byte {
        bus.write8(addr, cpu.reg(rd) as u8);
    } else {
        write_word(bus, addr, cpu.reg(rd));
    }
    cpu.add_cycles(1);
}

fn load_store_signed<M: Memory>(cpu: &mut Cpu, bus: &mut M, op: u32) {
    let h = (op >> 11) & 1 != 0;
    let s = (op >> 10) & 1 != 0;
    let ro = ((op >> 6) & 7) as usize;
    let rb = ((op >> 3) & 7) as usize;
    let rd = (op & 7) as usize;
    let addr = cpu.reg(rb).wrapping_add(cpu.reg(ro));

    match (h, s) {
        (false, false) => write_half(bus, addr, cpu.reg(rd) as u16),
        (true, false) => {
            let value = read_half(bus, addr);
            cpu.set_reg(rd, value);
        }
        (false, true) => {
            let value = bus.read8(addr) as i8 as i32 as u32;
            cpu.set_reg(rd, value);
        }
        (true, true) => {
            let value = read_signed_half(bus, addr);
            cpu.set_reg(rd, value);
        }
    }
    cpu.add_cycles(1);
}

fn load_store_immediate<M: Memory>(cpu: &mut Cpu, bus: &mut M, op: u32) {
    let byte = (op >> 12) & 1 != 0;
    let load = (op >> 11) & 1 != 0;
    let offset = (op >> 6) & 0x1F;
    let rb = ((op >> 3) & 7) as usize;
    let rd = (op & 7) as usize;
    let offset = if byte { offset } else { offset * 4 };
    let addr = cpu.reg(rb).wrapping_add(offset);

    if load {
        let value = if byte {
            bus.read8(addr) as u32
        } else {
            read_word(bus, addr)
        };
        cpu.set_reg(rd, value);
    } else if byte {
        bus.write8(addr, cpu.reg(rd) as u8);
    } else {
        write_word(bus, addr, cpu.reg(rd));
    }
    cpu.add_cycles(1);
}

fn load_store_halfword_immediate<M: Memory>(cpu: &mut Cpu, bus: &mut M, op: u32) {
    let load = (op >> 11) & 1 != 0;
    let offset = ((op >> 6) & 0x1F) * 2;
    let rb = ((op >> 3) & 7) as usize;
    let rd = (op & 7) as usize;
    let addr = cpu.reg(rb).wrapping_add(offset);

    if load {
        let value = read_half(bus, addr);
        cpu.set_reg(rd, value);
    } else {
        write_half(bus, addr, cpu.reg(rd) as u16);
    }
    cpu.add_cycles(1);
}

fn sp_relative<M: Memory>(cpu: &mut Cpu, bus: &mut M, op: u32) {
    let load = (op >> 11) & 1 != 0;
    let rd = ((op >> 8) & 7) as usize;
    let offset = (op & 0xFF) * 4;
    let addr = cpu.r[13].wrapping_add(offset);

    if load {
        cpu.set_reg(rd, read_word(bus, addr));
    } else {
        write_word(bus, addr, cpu.reg(rd));
    }
    cpu.add_cycles(1);
}

fn load_address(cpu: &mut Cpu, op: u32) {
    let sp = (op >> 11) & 1 != 0;
    let rd = ((op >> 8) & 7) as usize;
    let offset = (op & 0xFF) * 4;
    let base = if sp { cpu.r[13] } else { cpu.reg(15) & !2 };
    cpu.set_reg(rd, base.wrapping_add(offset));
}

fn add_sp(cpu: &mut Cpu, op: u32) {
    let subtract = (op >> 7) & 1 != 0;
    let offset = (op & 0x7F) * 4;
    let sp = cpu.r[13];
    cpu.r[13] = if subtract {
        sp.wrapping_sub(offset)
    } else {
        sp.wrapping_add(offset)
    };
}

fn push_pop<M: Memory>(cpu: &mut Cpu, bus: &mut M, op: u32) {
    let load = (op >> 11) & 1 != 0;
    let pc_lr = (op >> 8) & 1 != 0;
    let list = op & 0xFF;
    let count = list.count_ones() + pc_lr as u32;
    let bytes = count * 4;

    if load {
        let base = cpu.r[13];
        let mut addr = base;
        for i in 0..8 {
            if list & (1 << i) != 0 {
                let value = read_word(bus, addr);
                cpu.set_reg(i, value);
                addr += 4;
            }
        }
        if pc_lr {
            let value = read_word(bus, addr);
            cpu.set_reg(15, value);
        }
        cpu.r[13] = base.wrapping_add(bytes);
    } else {
        let base = cpu.r[13].wrapping_sub(bytes);
        let mut addr = base;
        for i in 0..8 {
            if list & (1 << i) != 0 {
                write_word(bus, addr, cpu.reg(i));
                addr += 4;
            }
        }
        if pc_lr {
            write_word(bus, addr, cpu.r[14]);
        }
        cpu.r[13] = base;
    }

    cpu.add_cycles(count);
}

fn multiple<M: Memory>(cpu: &mut Cpu, bus: &mut M, op: u32) {
    let load = (op >> 11) & 1 != 0;
    let rb = ((op >> 8) & 7) as usize;
    let list = op & 0xFF;
    let count = list.count_ones();
    let base = cpu.reg(rb);

    if list == 0 {
        // Thumb's empty-list quirk transfers PC and advances the base by 0x40.
        if load {
            let value = read_word(bus, base);
            cpu.set_reg(15, value);
        } else {
            write_word(bus, base, cpu.r[15].wrapping_add(4));
        }
        cpu.set_reg(rb, base.wrapping_add(0x40));
        cpu.add_cycles(1);
        return;
    }

    let mut addr = base;
    let new_base = base.wrapping_add(count * 4);
    let first_register = list.trailing_zeros() as usize;

    for i in 0..8 {
        if list & (1 << i) != 0 {
            if load {
                let value = read_word(bus, addr);
                cpu.set_reg(i, value);
            } else {
                // As with ARM, a base register that is transferred after the
                // first register sees the updated base. Thumb tests t230-t232.
                let value = if i == rb && rb != first_register {
                    new_base
                } else {
                    cpu.reg(i)
                };
                write_word(bus, addr, value);
            }
            addr += 4;
        }
    }

    if !(load && list & (1 << rb) != 0) {
        cpu.set_reg(rb, new_base);
    }
    cpu.add_cycles(count);
}

fn conditional_branch(cpu: &mut Cpu, op: u32) {
    let cond = (op >> 8) & 0xF;
    let offset = ((op & 0xFF) as i8 as i32 as u32) << 1;
    let target = cpu.reg(15).wrapping_add(offset);

    if cond == 0xE {
        cpu.undefined();
        return;
    }
    if cpu.condition_passed(cond) {
        cpu.set_reg(15, target);
        cpu.add_cycles(2);
    }
}

fn unconditional_branch(cpu: &mut Cpu, op: u32) {
    let offset = sign_extend_11(op & 0x7FF) << 1;
    let target = cpu.reg(15).wrapping_add(offset);
    cpu.set_reg(15, target);
    cpu.add_cycles(2);
}

fn long_branch(cpu: &mut Cpu, op: u32) {
    let hi = (op >> 11) & 1 != 0;

    if hi {
        // The low 11-bit field is unsigned; only the high field is signed.
        let low = (op & 0x7FF) << 1;
        let target = cpu.reg(14).wrapping_add(low);
        let return_addr = cpu.r[15] | 1;
        cpu.set_reg(14, return_addr);
        cpu.set_reg(15, target);
        cpu.add_cycles(2);
    } else {
        let high = sign_extend_11(op & 0x7FF) << 12;
        let target = cpu.reg(15).wrapping_add(high);
        cpu.set_reg(14, target);
    }
}

#[inline]
fn sign_extend_11(value: u32) -> u32 {
    (((value << 21) as i32) >> 21) as u32
}

fn read_word<M: Memory>(bus: &mut M, addr: u32) -> u32 {
    let value = bus.read32(addr & !3);
    if addr & 3 != 0 {
        value.rotate_right((addr & 3) * 8)
    } else {
        value
    }
}

fn write_word<M: Memory>(bus: &mut M, addr: u32, value: u32) {
    bus.write32(addr, value);
}

fn read_half<M: Memory>(bus: &mut M, addr: u32) -> u32 {
    let value = bus.read16(addr & !1) as u32;
    if addr & 1 != 0 {
        value.rotate_right(8)
    } else {
        value
    }
}

fn read_signed_half<M: Memory>(bus: &mut M, addr: u32) -> u32 {
    let value = bus.read16(addr & !1) as i16 as i32;
    if addr & 1 != 0 {
        (value >> 8) as u32
    } else {
        value as u32
    }
}

fn write_half<M: Memory>(bus: &mut M, addr: u32, value: u16) {
    bus.write16(addr, value);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cpu::tests_support::TestBus;

    #[test]
    fn thumb_mov_immediate_sets_flags() {
        let mut cpu = Cpu::new();
        cpu.set_thumb(true);
        let mut bus = TestBus::default();
        // MOV r0, #0  -> 0x2000
        bus.write16(0x0800_0000, 0x2000);
        cpu.r[15] = 0x0800_0000;
        cpu.step(&mut bus);
        assert_eq!(cpu.r[0], 0);
        assert!(cpu.flag_z());
    }

    #[test]
    fn thumb_add_immediate() {
        let mut cpu = Cpu::new();
        cpu.set_thumb(true);
        let mut bus = TestBus::default();
        cpu.r[0] = 41;
        // ADD r0, #1 -> 0x3001
        bus.write16(0x0800_0000, 0x3001);
        cpu.r[15] = 0x0800_0000;
        cpu.step(&mut bus);
        assert_eq!(cpu.r[0], 42);
    }

    #[test]
    fn thumb_bx_switches_to_arm() {
        let mut cpu = Cpu::new();
        cpu.set_thumb(true);
        let mut bus = TestBus::default();
        cpu.r[0] = 0x0800_0100;
        // BX r0 -> 0x4700
        bus.write16(0x0800_0000, 0x4700);
        cpu.r[15] = 0x0800_0000;
        cpu.step(&mut bus);
        assert!(!cpu.thumb());
        assert_eq!(cpu.r[15], 0x0800_0100);
    }
}
