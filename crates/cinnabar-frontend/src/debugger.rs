//! Interactive headless debugger.
//!
//! The debugger deliberately stays in the terminal: it is useful over SSH, in
//! CI logs, and while the core is still being brought up. The same state is
//! exposed through the core's peek and snapshot APIs, so a graphical frontend
//! can reuse it later.

use std::collections::BTreeSet;
use std::io::{self, Write};
use std::str::SplitWhitespace;

use anyhow::{Context, Result, bail};
use cinnabar_core::System;
use cinnabar_core::cpu::{FLAG_C, FLAG_F, FLAG_N, FLAG_V, FLAG_Z};
use cinnabar_core::disasm::{disassemble_arm, disassemble_thumb_pair, thumb_instruction_len};

const CONTINUE_STEP_LIMIT: u64 = 50_000_000;
const MAX_STEP_COUNT: u32 = 10_000_000;
const MAX_FRAME_COUNT: u32 = 3_600;
const MAX_DUMP_BYTES: u32 = 1_024;

pub fn run(mut system: System, title: &str) -> Result<System> {
    let mut debugger = Debugger::new(system);
    println!("Cinnabar GBA debugger");
    println!("ROM: {title}");
    println!("Type \"help\" for commands, \"q\" to quit.");
    debugger.print_state();

    loop {
        print!("cinnabar> ");
        io::stdout().flush().context("flush debugger prompt")?;
        let mut line = String::new();
        if io::stdin()
            .read_line(&mut line)
            .context("read debugger command")?
            == 0
        {
            break;
        }
        match debugger.command(line.trim()) {
            Ok(true) => {}
            Ok(false) => break,
            Err(error) => println!("error: {error:#}"),
        }
    }

    system = debugger.system;
    Ok(system)
}

struct Debugger {
    system: System,
    breakpoints: BTreeSet<u32>,
    steps: u64,
}

impl Debugger {
    fn new(system: System) -> Self {
        Self {
            system,
            breakpoints: BTreeSet::new(),
            steps: 0,
        }
    }

    fn command(&mut self, line: &str) -> Result<bool> {
        let mut parts = line.split_whitespace();
        let Some(command) = parts.next() else {
            return Ok(true);
        };

        match command {
            "h" | "help" | "?" => self.print_help(),
            "r" | "regs" => self.print_registers(),
            "d" | "disasm" | "dis" => {
                let address = optional_address(&mut parts, self.system.cpu.pc())?;
                let count = optional_count(&mut parts, 8)?.min(256);
                self.print_disassembly(address, count, self.system.cpu.thumb());
            }
            "da" | "darm" => {
                let address = optional_address(&mut parts, self.system.cpu.pc())?;
                let count = optional_count(&mut parts, 8)?.min(256);
                self.print_disassembly(address, count, false);
            }
            "dt" | "dthumb" => {
                let address = optional_address(&mut parts, self.system.cpu.pc())?;
                let count = optional_count(&mut parts, 8)?.min(256);
                self.print_disassembly(address, count, true);
            }
            "x" | "hex" | "mem" => {
                let address = optional_address(&mut parts, self.system.cpu.pc())?;
                let length = optional_count(&mut parts, 64)?.min(MAX_DUMP_BYTES);
                self.print_memory(address, length);
            }
            "s" | "step" => {
                let count = optional_count(&mut parts, 1)?.min(MAX_STEP_COUNT);
                self.step(count);
                self.print_state();
            }
            "f" | "frame" => {
                let count = optional_count(&mut parts, 1)?.min(MAX_FRAME_COUNT);
                self.run_frames(count);
                self.print_state();
            }
            "c" | "continue" | "run" => {
                self.continue_run();
                self.print_state();
            }
            "b" | "break" => {
                let address = required_address(&mut parts)?;
                self.breakpoints.insert(address);
                println!("breakpoint set at 0x{address:08X}");
            }
            "bd" | "delete" => {
                let address = required_address(&mut parts)?;
                if self.breakpoints.remove(&address) {
                    println!("breakpoint removed at 0x{address:08X}");
                } else {
                    println!("no breakpoint at 0x{address:08X}");
                }
            }
            "bl" | "breaks" => self.print_breakpoints(),
            "ppu" => self.print_ppu(),
            "dma" => self.print_dma(),
            "timers" => self.print_timers(),
            "irq" => self.print_irq(),
            "reset" => {
                self.system.reset();
                self.steps = 0;
                println!("system reset");
                self.print_state();
            }
            "q" | "quit" | "exit" => return Ok(false),
            _ => println!("unknown command \"{command}\"; type \"help\""),
        }
        Ok(true)
    }

    fn print_help(&self) {
        println!("Commands:");
        println!("  r, regs                 registers, CPSR and mode");
        println!("  d [addr] [count]        disassemble at addr/PC");
        println!("  da, dt [addr] [count]   disassemble as ARM or Thumb");
        println!("  x [addr] [length]       hex dump at addr/PC");
        println!("  s [n], step [n]         execute n instructions (default 1)");
        println!("  f [n], frame [n]        run n frames (breakpoints ignored)");
        println!("  c, continue             run until a breakpoint or the step limit");
        println!("  b <addr>, break <addr>  set a breakpoint");
        println!("  bd <addr>, delete       remove a breakpoint");
        println!("  bl, breaks              list breakpoints");
        println!("  ppu, dma, timers, irq   hardware state");
        println!("  reset                   reset the machine");
        println!("  q, quit, exit           leave the debugger");
    }

    fn print_state(&self) {
        self.print_registers();
        self.print_disassembly(self.system.cpu.pc(), 4, self.system.cpu.thumb());
    }

    fn print_registers(&self) {
        let registers = self.system.cpu.regs();
        for row in 0..4 {
            let base = row * 4;
            println!(
                "r{:<2} = {:08X}  r{:<2} = {:08X}  r{:<2} = {:08X}  r{:<2} = {:08X}",
                base,
                registers[base],
                base + 1,
                registers[base + 1],
                base + 2,
                registers[base + 2],
                base + 3,
                registers[base + 3],
            );
        }

        let cpsr = self.system.cpu.cpsr();
        let flags = [(FLAG_N, 'N'), (FLAG_Z, 'Z'), (FLAG_C, 'C'), (FLAG_V, 'V')]
            .iter()
            .map(|&(bit, name)| if cpsr & bit != 0 { name } else { '-' })
            .collect::<String>();
        println!(
            "cpsr={:08X}  mode={:?}  state={}  irq={}  fiq={}  flags={flags}",
            cpsr,
            self.system.cpu.mode(),
            if self.system.cpu.thumb() {
                "Thumb"
            } else {
                "ARM"
            },
            if self.system.cpu.irq_enabled() {
                "on"
            } else {
                "off"
            },
            if (cpsr & FLAG_F) == 0 { "on" } else { "off" },
        );
        println!(
            "steps={}  frame={}  vcount={}",
            self.steps, self.system.bus.ppu.frame_count, self.system.bus.ppu.vcount
        );
    }

    fn print_disassembly(&self, address: u32, count: u32, thumb: bool) {
        let mut address = if thumb { address & !1 } else { address & !3 };
        if thumb {
            for _ in 0..count {
                let first = self.system.peek16(address);
                let second = self.system.peek16(address.wrapping_add(2));
                let length = thumb_instruction_len(first);
                println!(
                    "{address:08X}  {first:04X}        {}",
                    disassemble_thumb_pair(first, second, address)
                );
                address = address.wrapping_add(length as u32);
            }
        } else {
            for _ in 0..count {
                let word = self.system.peek32(address);
                println!(
                    "{address:08X}  {word:08X}  {}",
                    disassemble_arm(word, address)
                );
                address = address.wrapping_add(4);
            }
        }
    }

    fn print_memory(&self, address: u32, length: u32) {
        let start = address & !0xF;
        let end = address.saturating_add(length);
        let mut row = start;
        while row < end {
            print!("{row:08X}: ");
            for offset in 0..16 {
                let current = row.wrapping_add(offset);
                if current < end {
                    print!("{:02X} ", self.system.peek8(current));
                } else {
                    print!("   ");
                }
            }
            print!(" |");
            for offset in 0..16 {
                let current = row.wrapping_add(offset);
                if current >= end {
                    break;
                }
                let byte = self.system.peek8(current);
                let character = if byte.is_ascii_graphic() || byte == b' ' {
                    byte as char
                } else {
                    '.'
                };
                print!("{character}");
            }
            println!("|");
            row = row.wrapping_add(16);
        }
    }

    fn step(&mut self, count: u32) {
        for _ in 0..count {
            self.system.step();
            self.steps += 1;
        }
    }

    fn run_frames(&mut self, count: u32) {
        for _ in 0..count {
            let _ = self.system.run_frame();
        }
    }

    fn continue_run(&mut self) {
        let mut skipped = Some(self.system.cpu.pc());
        let start_steps = self.steps;
        let mut hit_breakpoint = None;

        for _ in 0..CONTINUE_STEP_LIMIT {
            let pc = self.system.cpu.pc();
            if self.breakpoints.contains(&pc) && skipped != Some(pc) {
                hit_breakpoint = Some(pc);
                break;
            }
            skipped = None;
            self.system.step();
            self.steps += 1;
        }

        if let Some(pc) = hit_breakpoint {
            println!("breakpoint hit at 0x{pc:08X}");
        } else {
            println!(
                "continue stopped after {} steps (no breakpoint; step limit reached)",
                self.steps - start_steps
            );
        }
    }

    fn print_breakpoints(&self) {
        if self.breakpoints.is_empty() {
            println!("no breakpoints");
            return;
        }
        for address in &self.breakpoints {
            println!("0x{address:08X}");
        }
    }

    fn print_ppu(&self) {
        let ppu = &self.system.bus.ppu;
        println!(
            "PPU: dispcnt={:04X} dispstat={:04X} vcount={} frame_count={} line_cycles={}",
            ppu.dispcnt,
            ppu.dispstat,
            ppu.vcount,
            ppu.frame_count,
            ppu.line_cycles()
        );
        println!(
            "     bgcnt=[{}] bghofs=[{}] bgvofs=[{}]",
            hex_words(&ppu.bgcnt),
            hex_words(&ppu.bghofs),
            hex_words(&ppu.bgvofs)
        );
        println!(
            "     bldcnt={:04X} bldalpha={:04X} bldy={:04X} mosaic={:04X}",
            ppu.bldcnt, ppu.bldalpha, ppu.bldy, ppu.mosaic
        );
    }

    fn print_dma(&self) {
        for (index, channel) in self.system.bus.dma.channels.iter().enumerate() {
            println!(
                "DMA{index}: en={} timing={} src={:08X} dst={:08X} count={} control={:04X}",
                channel.enabled(),
                channel.timing(),
                channel.src,
                channel.dst,
                channel.count,
                channel.control
            );
        }
    }

    fn print_timers(&self) {
        for (index, timer) in self.system.bus.timers.timers.iter().enumerate() {
            println!(
                "TIM{index}: en={} reload={:04X} counter={:04X} control={:04X} prescaler={} cascade={}",
                timer.enabled(),
                timer.reload,
                timer.counter,
                timer.control,
                timer.prescaler(),
                timer.cascade()
            );
        }
    }

    fn print_irq(&self) {
        let irq = &self.system.bus.irq;
        println!(
            "IRQ: ie={:04X} if={:04X} ime={} pending={}",
            irq.ie,
            irq.if_,
            irq.ime,
            irq.pending()
        );
    }
}

fn hex_words(values: &[u16]) -> String {
    values
        .iter()
        .map(|value| format!("{value:04X}"))
        .collect::<Vec<_>>()
        .join(" ")
}

fn parse_address(text: &str) -> Result<u32> {
    let text = text.trim();
    let (digits, radix) =
        if let Some(rest) = text.strip_prefix("0x").or_else(|| text.strip_prefix("0X")) {
            (rest, 16)
        } else if let Some(rest) = text.strip_prefix('$') {
            (rest, 16)
        } else {
            (text, 10)
        };
    u32::from_str_radix(digits, radix).with_context(|| format!("invalid address \"{text}\""))
}

fn optional_address(parts: &mut SplitWhitespace, default: u32) -> Result<u32> {
    match parts.next() {
        Some(value) => parse_address(value),
        None => Ok(default),
    }
}

fn required_address(parts: &mut SplitWhitespace) -> Result<u32> {
    match parts.next() {
        Some(value) => parse_address(value),
        None => bail!("missing address"),
    }
}

fn optional_count(parts: &mut SplitWhitespace, default: u32) -> Result<u32> {
    match parts.next() {
        Some(value) => value
            .parse()
            .with_context(|| format!("invalid count \"{value}\"")),
        None => Ok(default),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cinnabar_core::Cartridge;

    fn debugger() -> Debugger {
        Debugger::new(System::new(Cartridge::new(cinnabar_testrom::demo_rom())))
    }

    #[test]
    fn parses_hex_and_decimal_addresses() {
        assert_eq!(parse_address("0x08000000").unwrap(), 0x0800_0000);
        assert_eq!(parse_address("$8000000").unwrap(), 0x0800_0000);
        assert_eq!(parse_address("134217728").unwrap(), 0x0800_0000);
        assert!(parse_address("nope").is_err());
    }

    #[test]
    fn step_advances_the_cpu() {
        let mut debugger = debugger();
        let before = debugger.system.cpu.pc();
        debugger.step(1);
        assert_ne!(debugger.system.cpu.pc(), before);
    }

    #[test]
    fn breakpoints_can_be_added_and_removed() {
        let mut debugger = debugger();
        debugger.command("b 0x08000000").unwrap();
        assert!(debugger.breakpoints.contains(&0x0800_0000));
        debugger.command("bd 0x08000000").unwrap();
        assert!(debugger.breakpoints.is_empty());
    }

    #[test]
    fn continue_stops_at_a_breakpoint() {
        let mut debugger = debugger();
        // The demo ROM's reset vector branches here from 0x08000000.
        let branch_target = 0x0800_00C0;
        debugger.breakpoints.insert(branch_target);
        debugger.continue_run();
        assert_eq!(debugger.system.cpu.pc(), branch_target);
        assert!(debugger.steps > 0);
    }

    #[test]
    fn a_frame_advances_the_ppu() {
        let mut debugger = debugger();
        let before = debugger.system.bus.ppu.frame_count;
        debugger.run_frames(1);
        assert!(debugger.system.bus.ppu.frame_count > before);
    }
}
