//! ARM and Thumb instruction disassembly for the headless debugger.
//!
//! This is a readable, best-effort decoder for the instruction families the
//! debugger is likely to encounter. Unknown encodings are printed as raw
//! words or halfwords instead of being guessed.

const CONDITIONS: [&str; 16] = [
    "eq", "ne", "cs", "cc", "mi", "pl", "vs", "vc", "hi", "ls", "ge", "lt", "gt", "le", "", "nv",
];

const DATA_OPS: [&str; 16] = [
    "and", "eor", "sub", "rsb", "add", "adc", "sbc", "rsc", "tst", "teq", "cmp", "cmn", "orr",
    "mov", "bic", "mvn",
];

pub fn disassemble_arm(word: u32, address: u32) -> String {
    let condition = (word >> 28) & 0xF;
    if condition == 0xF {
        return raw_arm(word);
    }
    let suffix = CONDITIONS[condition as usize];

    if let Some(text) = arm_multiply(word, suffix) {
        return text;
    }
    if let Some(text) = arm_bx(word, suffix) {
        return text;
    }
    if let Some(text) = arm_psr(word, suffix) {
        return text;
    }
    if let Some(text) = arm_swap(word, suffix) {
        return text;
    }
    if let Some(text) = arm_halfword(word, suffix) {
        return text;
    }
    if let Some(text) = arm_data_processing(word, suffix) {
        return text;
    }
    if let Some(text) = arm_single_transfer(word, suffix) {
        return text;
    }
    if let Some(text) = arm_block_transfer(word, suffix) {
        return text;
    }
    if let Some(text) = arm_branch(word, address, suffix) {
        return text;
    }
    if word & 0x0F00_0000 == 0x0F00_0000 {
        return format!("swi{suffix} #0x{:X}", word & 0x00FF_FFFF);
    }
    raw_arm(word)
}

fn raw_arm(word: u32) -> String {
    format!(".word 0x{word:08X}")
}

fn condition_suffix(condition: impl Into<u32>) -> &'static str {
    CONDITIONS[condition.into() as usize]
}

fn reg_name(register: impl Into<u32>) -> String {
    let register = register.into();
    match register {
        13 => "sp".to_string(),
        14 => "lr".to_string(),
        15 => "pc".to_string(),
        _ => format!("r{register}"),
    }
}

fn register_list(list: u16) -> String {
    let mut parts = Vec::new();
    let mut index: u32 = 0;
    while index < 16 {
        if list & (1u16 << index) == 0 {
            index += 1;
            continue;
        }
        let start = index;
        while index < 16 && list & (1u16 << index) != 0 {
            index += 1;
        }
        let end = index - 1;
        if start == end {
            parts.push(reg_name(start));
        } else if end == start + 1 {
            parts.push(reg_name(start));
            parts.push(reg_name(end));
        } else {
            parts.push(format!("{}-{}", reg_name(start), reg_name(end)));
        }
    }
    format!("{{{}}}", parts.join(", "))
}

fn shift_name(shift: u32) -> &'static str {
    match shift {
        0 => "lsl",
        1 => "lsr",
        2 => "asr",
        _ => "ror",
    }
}

fn arm_immediate(word: u32) -> u32 {
    let immediate = word & 0xFF;
    let rotate = ((word >> 8) & 0xF) * 2;
    immediate.rotate_right(rotate)
}

fn arm_shifted_register(word: u32) -> String {
    let rm = word & 0xF;
    let shift = (word >> 5) & 3;
    if word & (1 << 4) != 0 {
        let rs = (word >> 8) & 0xF;
        return format!("{}, {} {}", reg_name(rm), shift_name(shift), reg_name(rs));
    }

    let amount = (word >> 7) & 0x1F;
    match (shift, amount) {
        (0, 0) => reg_name(rm),
        (1 | 2, 0) => format!("{}, {} #32", reg_name(rm), shift_name(shift)),
        (3, 0) => format!("{}, rrx", reg_name(rm)),
        _ => format!("{}, {} #{amount}", reg_name(rm), shift_name(shift)),
    }
}

fn arm_operand2(word: u32) -> String {
    if word & (1 << 25) != 0 {
        return format!("#0x{:X}", arm_immediate(word));
    }
    arm_shifted_register(word)
}

fn negate_offset(offset: &str) -> String {
    if let Some(rest) = offset.strip_prefix('#') {
        format!("#-{rest}")
    } else {
        format!("-{offset}")
    }
}

fn arm_data_processing(word: u32, suffix: &str) -> Option<String> {
    if word & 0x0C00_0000 != 0 {
        return None;
    }

    let opcode = (word >> 21) & 0xF;
    let set_flags = word & (1 << 20) != 0;
    let rn = (word >> 16) & 0xF;
    let rd = (word >> 12) & 0xF;
    let operand = arm_operand2(word);
    let name = DATA_OPS[opcode as usize];

    let text = if (8..=11).contains(&opcode) {
        format!("{name}{suffix} {}, {operand}", reg_name(rn))
    } else if opcode == 13 || opcode == 15 {
        let flags = if set_flags { "s" } else { "" };
        format!("{name}{suffix}{flags} {}, {operand}", reg_name(rd))
    } else {
        let flags = if set_flags { "s" } else { "" };
        format!(
            "{name}{suffix}{flags} {}, {}, {operand}",
            reg_name(rd),
            reg_name(rn)
        )
    };
    Some(text)
}

fn arm_multiply(word: u32, suffix: &str) -> Option<String> {
    if word & 0x0F80_00F0 == 0x0080_0090 {
        let opcode = (word >> 21) & 0xF;
        let name = match opcode {
            4 => "umull",
            5 => "umlal",
            6 => "smull",
            7 => "smlal",
            _ => return None,
        };
        let set_flags = word & (1 << 20) != 0;
        let flags = if set_flags { "s" } else { "" };
        let rd_high = (word >> 16) & 0xF;
        let rd_low = (word >> 12) & 0xF;
        let rs = (word >> 8) & 0xF;
        let rm = word & 0xF;
        return Some(format!(
            "{name}{suffix}{flags} {}, {}, {}, {}",
            reg_name(rd_low),
            reg_name(rd_high),
            reg_name(rm),
            reg_name(rs)
        ));
    }

    if word & 0x0FC0_00F0 != 0x0000_0090 {
        return None;
    }

    let opcode = (word >> 21) & 0xF;
    let set_flags = word & (1 << 20) != 0;
    let rd = (word >> 16) & 0xF;
    let rn = (word >> 12) & 0xF;
    let rs = (word >> 8) & 0xF;
    let rm = word & 0xF;
    let flags = if set_flags { "s" } else { "" };

    let text = match opcode {
        0 => format!(
            "mul{suffix}{flags} {}, {}, {}",
            reg_name(rd),
            reg_name(rm),
            reg_name(rs)
        ),
        1 => format!(
            "mla{suffix}{flags} {}, {}, {}, {}",
            reg_name(rd),
            reg_name(rm),
            reg_name(rs),
            reg_name(rn)
        ),
        _ => return None,
    };
    Some(text)
}

fn arm_bx(word: u32, suffix: &str) -> Option<String> {
    if word & 0x0FFF_FFF0 == 0x012F_FF10 {
        return Some(format!("bx{suffix} {}", reg_name(word & 0xF)));
    }
    if word & 0x0FFF_FFF0 == 0x012F_FF30 {
        return Some(format!("blx{suffix} {}", reg_name(word & 0xF)));
    }
    None
}

fn arm_psr(word: u32, suffix: &str) -> Option<String> {
    if word & 0x0FBF_0FFF == 0x010F_0000 {
        let rd = (word >> 12) & 0xF;
        let source = if word & (1 << 22) != 0 {
            "spsr"
        } else {
            "cpsr"
        };
        return Some(format!("mrs{suffix} {}, {source}", reg_name(rd)));
    }

    if word & 0x0FB0_FFF0 == 0x0120_F000 {
        let mask = (word >> 16) & 0xF;
        let rm = word & 0xF;
        let source = if word & (1 << 22) != 0 {
            "spsr"
        } else {
            "cpsr"
        };
        return Some(format!(
            "msr{suffix} {source}{}, {}",
            psr_mask(mask),
            reg_name(rm)
        ));
    }

    if word & 0x0FB0_F000 == 0x0320_F000 {
        let mask = (word >> 16) & 0xF;
        let immediate = arm_immediate(word);
        let source = if word & (1 << 22) != 0 {
            "spsr"
        } else {
            "cpsr"
        };
        return Some(format!(
            "msr{suffix} {source}{}, #0x{immediate:X}",
            psr_mask(mask)
        ));
    }

    None
}

fn psr_mask(mask: u32) -> String {
    let mut parts = String::new();
    if mask & 1 != 0 {
        parts.push('c');
    }
    if mask & 2 != 0 {
        parts.push('x');
    }
    if mask & 4 != 0 {
        parts.push('s');
    }
    if mask & 8 != 0 {
        parts.push('f');
    }
    if parts.is_empty() {
        String::new()
    } else {
        format!("_{parts}")
    }
}

fn arm_swap(word: u32, suffix: &str) -> Option<String> {
    if word & 0x0FB0_0FF0 != 0x0100_0090 {
        return None;
    }
    let byte = if word & (1 << 22) != 0 { "b" } else { "" };
    let rn = (word >> 16) & 0xF;
    let rd = (word >> 12) & 0xF;
    let rm = word & 0xF;
    Some(format!(
        "swp{suffix}{byte} {}, {}, [{}]",
        reg_name(rd),
        reg_name(rm),
        reg_name(rn)
    ))
}

fn arm_halfword(word: u32, suffix: &str) -> Option<String> {
    if word & 0x0E00_0090 != 0x0000_0090 || (word >> 5) & 3 == 0 {
        return None;
    }

    let pre = word & (1 << 24) != 0;
    let up = word & (1 << 23) != 0;
    let immediate = word & (1 << 22) != 0;
    let writeback = word & (1 << 21) != 0;
    let load = word & (1 << 20) != 0;
    let rn = (word >> 16) & 0xF;
    let rd = (word >> 12) & 0xF;
    let kind = (word >> 5) & 3;

    let name = match (kind, load) {
        (1, true) => "ldrh",
        (1, false) => "strh",
        (2, true) => "ldrsb",
        (3, true) => "ldrsh",
        _ => return None,
    };

    let offset = if immediate {
        let high = (word >> 4) & 0xF0;
        format!("#0x{:X}", high | (word & 0xF))
    } else {
        reg_name(word & 0xF)
    };
    let offset = if up { offset } else { negate_offset(&offset) };

    let address = if pre {
        let mut address = if offset == "#0x0" {
            format!("[{}]", reg_name(rn))
        } else {
            format!("[{}, {offset}]", reg_name(rn))
        };
        if writeback {
            address.push('!');
        }
        address
    } else {
        format!("[{}], {offset}", reg_name(rn))
    };

    Some(format!("{name}{suffix} {}, {address}", reg_name(rd)))
}

fn arm_single_transfer(word: u32, suffix: &str) -> Option<String> {
    if word & 0x0C00_0000 != 0x0400_0000 {
        return None;
    }

    let register_offset = word & (1 << 25) != 0;
    let pre = word & (1 << 24) != 0;
    let up = word & (1 << 23) != 0;
    let byte = word & (1 << 22) != 0;
    let writeback = word & (1 << 21) != 0;
    let load = word & (1 << 20) != 0;
    let rn = (word >> 16) & 0xF;
    let rd = (word >> 12) & 0xF;

    let name = if load { "ldr" } else { "str" };
    let byte = if byte { "b" } else { "" };
    let offset = if register_offset {
        arm_shifted_register(word)
    } else {
        format!("#0x{:X}", word & 0xFFF)
    };
    let offset = if up { offset } else { negate_offset(&offset) };

    let address = if pre {
        let mut address = if offset == "#0x0" {
            format!("[{}]", reg_name(rn))
        } else {
            format!("[{}, {offset}]", reg_name(rn))
        };
        if writeback {
            address.push('!');
        }
        address
    } else {
        format!("[{}], {offset}", reg_name(rn))
    };

    Some(format!("{name}{suffix}{byte} {}, {address}", reg_name(rd)))
}

fn arm_block_transfer(word: u32, suffix: &str) -> Option<String> {
    if word & 0x0E00_0000 != 0x0800_0000 {
        return None;
    }

    let pre = word & (1 << 24) != 0;
    let up = word & (1 << 23) != 0;
    let user = word & (1 << 22) != 0;
    let writeback = word & (1 << 21) != 0;
    let load = word & (1 << 20) != 0;
    let rn = (word >> 16) & 0xF;
    let list = register_list((word & 0xFFFF) as u16);

    if rn == 13 && !user {
        if !load && pre && !up && writeback {
            return Some(format!("push{suffix} {list}"));
        }
        if load && !pre && up && writeback {
            return Some(format!("pop{suffix} {list}"));
        }
    }

    let mode = match (pre, up) {
        (false, true) => "ia",
        (true, true) => "ib",
        (false, false) => "da",
        (true, false) => "db",
    };
    let mnemonic = if load { "ldm" } else { "stm" };
    let mut address = reg_name(rn);
    if writeback {
        address.push('!');
    }
    let user = if user { "^" } else { "" };
    Some(format!("{mnemonic}{suffix}{mode} {address}, {list}{user}"))
}

fn arm_branch(word: u32, address: u32, suffix: &str) -> Option<String> {
    if word & 0x0E00_0000 != 0x0A00_0000 {
        return None;
    }
    let link = word & (1 << 24) != 0;
    let immediate = (word & 0x00FF_FFFF) as i32;
    let immediate = if immediate & 0x0080_0000 != 0 {
        immediate - 0x0100_0000
    } else {
        immediate
    };
    let target = address
        .wrapping_add(8)
        .wrapping_add((immediate << 2) as u32);
    let name = if link { "bl" } else { "b" };
    Some(format!("{name}{suffix} 0x{target:08X}"))
}

pub fn disassemble_thumb(halfword: u16, address: u32) -> String {
    disassemble_thumb_pair(halfword, 0, address)
}

/// Disassemble a Thumb instruction. `second` supplies the following halfword
/// for the two-halfword `bl`/`blx` encodings; other instructions ignore it.
pub fn disassemble_thumb_pair(halfword: u16, second: u16, address: u32) -> String {
    let h = halfword;
    let rd = h & 7;
    let rs = (h >> 3) & 7;

    if h & 0xF800 == 0xF000 && second & 0xF800 == 0xF800 {
        return thumb_long_branch(h, second, address);
    }

    if h & 0xE000 == 0 {
        if h & 0x1800 != 0x1800 {
            let op = (h >> 11) & 3;
            let offset = (h >> 6) & 0x1F;
            let name = match op {
                0 => "lsls",
                1 => "lsrs",
                2 => "asrs",
                _ => return format!(".hword 0x{h:04X}"),
            };
            let offset = if offset == 0 && op != 0 {
                "32".to_string()
            } else {
                offset.to_string()
            };
            return format!("{name} {}, {}, #{offset}", reg_name(rd), reg_name(rs));
        }

        let immediate = h & (1 << 10) != 0;
        let subtract = h & (1 << 9) != 0;
        let name = if subtract { "subs" } else { "adds" };
        if immediate {
            let value = (h >> 6) & 7;
            return format!("{name} {}, {}, #{value}", reg_name(rd), reg_name(rs));
        }
        let rn = (h >> 6) & 7;
        return format!(
            "{name} {}, {}, {}",
            reg_name(rd),
            reg_name(rs),
            reg_name(rn)
        );
    }

    if h & 0xE000 == 0x2000 {
        let op = (h >> 11) & 3;
        let rd = (h >> 8) & 7;
        let immediate = h & 0xFF;
        let name = match op {
            0 => "movs",
            1 => "cmp",
            2 => "adds",
            _ => "subs",
        };
        if op == 0 {
            return format!("{name} {}, #0x{immediate:X}", reg_name(rd));
        }
        return format!("{name} {}, #0x{immediate:X}", reg_name(rd));
    }

    if h & 0xFC00 == 0x4000 {
        let op = (h >> 6) & 0xF;
        let rd = h & 7;
        let rs = (h >> 3) & 7;
        let name = match op {
            0 => "ands",
            1 => "eors",
            2 => "lsls",
            3 => "lsrs",
            4 => "asrs",
            5 => "adcs",
            6 => "sbcs",
            7 => "rors",
            8 => "tst",
            9 => "negs",
            10 => "cmp",
            11 => "cmn",
            12 => "orrs",
            13 => "muls",
            14 => "bics",
            _ => "mvns",
        };
        return match op {
            8 | 10 | 11 => format!("{name} {}, {}", reg_name(rd), reg_name(rs)),
            9 => format!("{name} {}, {}", reg_name(rd), reg_name(rs)),
            _ => format!("{name} {}, {}", reg_name(rd), reg_name(rs)),
        };
    }

    if h & 0xFC00 == 0x4400 {
        let op = (h >> 8) & 3;
        let high_rd = (h >> 7) & 1;
        let high_rs = (h >> 6) & 1;
        let rd = (h & 7) | (high_rd << 3);
        let rs = ((h >> 3) & 7) | (high_rs << 3);
        let name = match op {
            0 => "add",
            1 => "cmp",
            2 => "mov",
            _ => {
                if high_rd == 0 {
                    "bx"
                } else {
                    "blx"
                }
            }
        };
        if op == 3 {
            return format!("{name} {}", reg_name(rs));
        }
        return format!("{name} {}, {}", reg_name(rd), reg_name(rs));
    }

    if h & 0xF800 == 0x4800 {
        let rd = (h >> 8) & 7;
        let offset = u32::from(h & 0xFF) * 4;
        let target = (address.wrapping_add(4) & !3).wrapping_add(offset);
        return format!("ldr {}, [pc, #0x{offset:X}] ; 0x{target:08X}", reg_name(rd));
    }

    if h & 0xF000 == 0x5000 {
        let load = h & (1 << 11) != 0;
        let byte = h & (1 << 10) != 0;
        let register_offset = h & (1 << 9) != 0;
        let rb = (h >> 3) & 7;
        let ro = (h >> 6) & 7;
        let rd = h & 7;

        if register_offset {
            let signed = h & (1 << 10) != 0;
            let half = h & (1 << 11) != 0;
            let name = if signed {
                if half { "ldrsh" } else { "ldrsb" }
            } else if load {
                "ldrh"
            } else {
                "strh"
            };
            return format!(
                "{name} {}, [{}, {}]",
                reg_name(rd),
                reg_name(rb),
                reg_name(ro)
            );
        }

        let name = match (load, byte) {
            (true, false) => "ldr",
            (true, true) => "ldrb",
            (false, false) => "str",
            (false, true) => "strb",
        };
        return format!(
            "{name} {}, [{}, {}]",
            reg_name(rd),
            reg_name(rb),
            reg_name(ro)
        );
    }

    if h & 0xE000 == 0x6000 {
        let byte = h & (1 << 12) != 0;
        let load = h & (1 << 11) != 0;
        let offset = u32::from((h >> 6) & 0x1F);
        let offset = if byte { offset } else { offset * 4 };
        let rb = (h >> 3) & 7;
        let rd = h & 7;
        let name = match (load, byte) {
            (true, false) => "ldr",
            (true, true) => "ldrb",
            (false, false) => "str",
            (false, true) => "strb",
        };
        let address = if offset == 0 {
            format!("[{}]", reg_name(rb))
        } else {
            format!("[{}, #0x{:X}]", reg_name(rb), offset)
        };
        return format!("{name} {}, {address}", reg_name(rd));
    }

    if h & 0xF000 == 0x8000 {
        let load = h & (1 << 11) != 0;
        let offset = u32::from((h >> 6) & 0x1F) * 2;
        let rb = (h >> 3) & 7;
        let rd = h & 7;
        let name = if load { "ldrh" } else { "strh" };
        let address = if offset == 0 {
            format!("[{}]", reg_name(rb))
        } else {
            format!("[{}, #0x{offset:X}]", reg_name(rb))
        };
        return format!("{name} {}, {address}", reg_name(rd));
    }

    if h & 0xF000 == 0x9000 {
        let load = h & (1 << 11) != 0;
        let rd = (h >> 8) & 7;
        let offset = u32::from(h & 0xFF) * 4;
        let name = if load { "ldr" } else { "str" };
        let address = if offset == 0 {
            "[sp]".to_string()
        } else {
            format!("[sp, #0x{offset:X}]")
        };
        return format!("{name} {}, {address}", reg_name(rd));
    }

    if h & 0xF000 == 0xA000 {
        let use_sp = h & (1 << 11) != 0;
        let rd = (h >> 8) & 7;
        let offset = u32::from(h & 0xFF) * 4;
        if use_sp {
            return format!("add {}, sp, #0x{offset:X}", reg_name(rd));
        }
        let target = (address.wrapping_add(4) & !3).wrapping_add(offset);
        return format!("add {}, pc, #0x{offset:X} ; 0x{target:08X}", reg_name(rd));
    }

    if h & 0xFF00 == 0xB000 {
        let negative = h & (1 << 7) != 0;
        let offset = u32::from(h & 0x7F) * 4;
        let name = if negative { "sub" } else { "add" };
        return format!("{name} sp, #0x{offset:X}");
    }

    if h & 0xF600 == 0xB400 {
        let load = h & (1 << 11) != 0;
        let extra = h & (1 << 8) != 0;
        let mut list = h & 0xFF;
        if extra {
            list |= if load { 1 << 15 } else { 1 << 14 };
        }
        let name = if load { "pop" } else { "push" };
        return format!("{name} {}", register_list(list));
    }

    if h & 0xF000 == 0xC000 {
        let load = h & (1 << 11) != 0;
        let rb = (h >> 8) & 7;
        let list = register_list(h & 0xFF);
        let name = if load { "ldmia" } else { "stmia" };
        return format!("{name} {}!, {list}", reg_name(rb));
    }

    if h & 0xF000 == 0xD000 {
        let condition = (h >> 8) & 0xF;
        if condition == 0xF {
            return format!("swi #0x{:X}", h & 0xFF);
        }
        if condition == 0xE {
            return format!(".hword 0x{h:04X}");
        }
        let offset = ((h & 0xFF) as i8 as i32) << 1;
        let target = address.wrapping_add(4).wrapping_add(offset as u32);
        return format!("b{} 0x{target:08X}", condition_suffix(condition));
    }

    if h & 0xF800 == 0xE000 {
        let offset = (((h & 0x7FF) as i32) << 21 >> 20) as u32;
        let target = address.wrapping_add(4).wrapping_add(offset);
        return format!("b 0x{target:08X}");
    }

    if h & 0xF000 == 0xF000 {
        return format!(".hword 0x{h:04X}");
    }

    format!(".hword 0x{h:04X}")
}

/// Number of bytes occupied by the Thumb instruction starting at `halfword`.
pub fn thumb_instruction_len(halfword: u16) -> usize {
    if halfword & 0xF800 == 0xF000 { 4 } else { 2 }
}

fn thumb_long_branch(first: u16, second: u16, address: u32) -> String {
    let s = ((first >> 10) & 1) as u32;
    let imm10 = (first & 0x3FF) as u32;
    let j1 = ((second >> 13) & 1) as u32;
    let j2 = ((second >> 11) & 1) as u32;
    let imm11 = (second & 0x7FF) as u32;
    let i1 = (!(j1 ^ s)) & 1;
    let i2 = (!(j2 ^ s)) & 1;
    let mut offset = (s << 24) | (i1 << 23) | (i2 << 22) | (imm10 << 12) | (imm11 << 1);
    if offset & (1 << 24) != 0 {
        offset |= 0xFE00_0000;
    }
    let target = address.wrapping_add(4).wrapping_add(offset);
    format!("bl 0x{target:08X}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arm_common_instructions() {
        assert_eq!(disassemble_arm(0xE3A0_0001, 0x0800_0000), "mov r0, #0x1");
        assert_eq!(disassemble_arm(0xE080_0001, 0x0800_0000), "add r0, r0, r1");
        assert_eq!(disassemble_arm(0xE352_0010, 0x0800_0000), "cmp r2, #0x10");
        assert_eq!(
            disassemble_arm(0xE594_3008, 0x0800_0000),
            "ldr r3, [r4, #0x8]"
        );
        assert_eq!(
            disassemble_arm(0xE566_5001, 0x0800_0000),
            "strb r5, [r6, #-0x1]!"
        );
        assert_eq!(
            disassemble_arm(0xE1D8_70B2, 0x0800_0000),
            "ldrh r7, [r8, #0x2]"
        );
        assert_eq!(
            disassemble_arm(0xE794_3105, 0x0800_0000),
            "ldr r3, [r4, r5, lsl #2]"
        );
        assert_eq!(
            disassemble_arm(0xE704_3005, 0x0800_0000),
            "str r3, [r4, -r5]"
        );
        assert_eq!(
            disassemble_arm(0xE198_70B9, 0x0800_0000),
            "ldrh r7, [r8, r9]"
        );
        assert_eq!(
            disassemble_arm(0xE92D_400F, 0x0800_0000),
            "push {r0-r3, lr}"
        );
        assert_eq!(disassemble_arm(0xE8BD_800F, 0x0800_0000), "pop {r0-r3, pc}");
        assert_eq!(
            disassemble_arm(0xE8B0_0006, 0x0800_0000),
            "ldmia r0!, {r1, r2}"
        );
        assert_eq!(disassemble_arm(0xE12F_FF1E, 0x0800_0000), "bx lr");
        assert_eq!(disassemble_arm(0xE000_0291, 0x0800_0000), "mul r0, r1, r2");
        assert_eq!(
            disassemble_arm(0xE020_3291, 0x0800_0000),
            "mla r0, r1, r2, r3"
        );
        assert_eq!(
            disassemble_arm(0xE081_0392, 0x0800_0000),
            "umull r0, r1, r2, r3"
        );
        assert_eq!(
            disassemble_arm(0xE0C5_4796, 0x0800_0000),
            "smull r4, r5, r6, r7"
        );
        assert_eq!(disassemble_arm(0xEF00_0123, 0x0800_0000), "swi #0x123");
    }

    #[test]
    fn arm_branches_use_the_pipeline_bias() {
        assert_eq!(disassemble_arm(0xEA00_0000, 0x0800_0000), "b 0x08000008");
        assert_eq!(disassemble_arm(0xEB00_0000, 0x0800_0000), "bl 0x08000008");
    }

    #[test]
    fn arm_psr_and_swap_instructions() {
        assert_eq!(disassemble_arm(0xE10F_0000, 0x0800_0000), "mrs r0, cpsr");
        assert_eq!(disassemble_arm(0xE121_F000, 0x0800_0000), "msr cpsr_c, r0");
        assert_eq!(
            disassemble_arm(0xE102_0091, 0x0800_0000),
            "swp r0, r1, [r2]"
        );
    }

    #[test]
    fn thumb_common_instructions() {
        assert_eq!(disassemble_thumb(0x2001, 0x0800_0000), "movs r0, #0x1");
        assert_eq!(disassemble_thumb(0x3102, 0x0800_0000), "adds r1, #0x2");
        assert_eq!(disassemble_thumb(0x191A, 0x0800_0000), "adds r2, r3, r4");
        assert_eq!(disassemble_thumb(0x00C8, 0x0800_0000), "lsls r0, r1, #3");
        assert_eq!(disassemble_thumb(0x4008, 0x0800_0000), "ands r0, r1");
        assert_eq!(disassemble_thumb(0x4208, 0x0800_0000), "tst r0, r1");
        assert_eq!(disassemble_thumb(0x4288, 0x0800_0000), "cmp r0, r1");
        assert_eq!(disassemble_thumb(0x42C8, 0x0800_0000), "cmn r0, r1");
        assert_eq!(disassemble_thumb(0x4680, 0x0800_0000), "mov r8, r0");
        assert_eq!(disassemble_thumb(0x4580, 0x0800_0000), "cmp r8, r0");
        assert_eq!(disassemble_thumb(0x4770, 0x0800_0000), "bx lr");
        assert_eq!(
            disassemble_thumb(0x4801, 0x0800_0000),
            "ldr r0, [pc, #0x4] ; 0x08000008"
        );
        assert_eq!(disassemble_thumb(0x6891, 0x0800_0000), "ldr r1, [r2, #0x8]");
        assert_eq!(disassemble_thumb(0x6091, 0x0800_0000), "str r1, [r2, #0x8]");
        assert_eq!(
            disassemble_thumb(0x7A11, 0x0800_0000),
            "ldrb r1, [r2, #0x8]"
        );
        assert_eq!(
            disassemble_thumb(0x8911, 0x0800_0000),
            "ldrh r1, [r2, #0x8]"
        );
        assert_eq!(disassemble_thumb(0x5AD1, 0x0800_0000), "ldrh r1, [r2, r3]");
        assert_eq!(disassemble_thumb(0x52D1, 0x0800_0000), "strh r1, [r2, r3]");
        assert_eq!(disassemble_thumb(0x56D1, 0x0800_0000), "ldrsb r1, [r2, r3]");
        assert_eq!(disassemble_thumb(0x5ED1, 0x0800_0000), "ldrsh r1, [r2, r3]");
        assert_eq!(disassemble_thumb(0xB50F, 0x0800_0000), "push {r0-r3, lr}");
        assert_eq!(disassemble_thumb(0xBD0F, 0x0800_0000), "pop {r0-r3, pc}");
        assert_eq!(
            disassemble_thumb(0xC006, 0x0800_0000),
            "stmia r0!, {r1, r2}"
        );
        assert_eq!(disassemble_thumb(0xE000, 0x0800_0000), "b 0x08000004");
        assert_eq!(disassemble_thumb(0xD000, 0x0800_0000), "beq 0x08000004");
        assert_eq!(disassemble_thumb(0xDF12, 0x0800_0000), "swi #0x12");
    }

    #[test]
    fn thumb_bl_pair() {
        assert_eq!(
            disassemble_thumb_pair(0xF000, 0xF800, 0x0800_0000),
            "bl 0x08000004"
        );
        assert_eq!(
            disassemble_thumb_pair(0xF000, 0xF801, 0x0800_0000),
            "bl 0x08000006"
        );
    }
}
