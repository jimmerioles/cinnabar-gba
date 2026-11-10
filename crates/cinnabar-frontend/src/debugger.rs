//! Interactive headless debugger.
//!
//! The debugger deliberately stays in the terminal: it is useful over SSH, in
//! CI logs, and while the core is still being brought up. The same state is
//! exposed through the core's peek and snapshot APIs, so a graphical frontend
//! can reuse it later.

use std::collections::{BTreeMap, BTreeSet, VecDeque};
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
const DEFAULT_REWIND_LIMIT: usize = 120;

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
    save_slots: BTreeMap<u8, Vec<u8>>,
    rewind: VecDeque<Vec<u8>>,
    rewind_limit: usize,
    recording: bool,
}

impl Debugger {
    fn new(system: System) -> Self {
        Self {
            system,
            breakpoints: BTreeSet::new(),
            steps: 0,
            save_slots: BTreeMap::new(),
            rewind: VecDeque::new(),
            rewind_limit: DEFAULT_REWIND_LIMIT,
            recording: false,
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
            "ss" | "save-state" => {
                let slot = optional_slot(&mut parts)?;
                let state = self.system.save_state().context("save state")?;
                let size = state.len();
                self.save_slots.insert(slot, state);
                println!("saved state {slot} ({size} bytes)");
            }
            "sl" | "load-state" => {
                let slot = optional_slot(&mut parts)?;
                self.load_slot(slot)?;
            }
            "states" => self.print_states(),
            "record" => self.record_command(parts.next())?,
            "rewind" => {
                let count = optional_count(&mut parts, 1)?.min(1_000) as usize;
                self.rewind_frames(count)?;
                self.print_state();
            }
            "w" | "w16" => {
                let address = required_address(&mut parts)?;
                let value = required_value(&mut parts)?;
                let value = u16::try_from(value).context("value is larger than 16 bits")?;
                self.system.poke16(address, value);
                println!("wrote 0x{value:04X} to 0x{address:08X}");
            }
            "w8" => {
                let address = required_address(&mut parts)?;
                let value = required_value(&mut parts)?;
                let value = u8::try_from(value).context("value is larger than 8 bits")?;
                self.system.poke8(address, value);
                println!("wrote 0x{value:02X} to 0x{address:08X}");
            }
            "w32" => {
                let address = required_address(&mut parts)?;
                let value = required_value(&mut parts)?;
                self.system.poke32(address, value);
                println!("wrote 0x{value:08X} to 0x{address:08X}");
            }
            "palette" | "pal" => {
                let start = optional_count(&mut parts, 0)?.min(255);
                let count = optional_count(&mut parts, 32)?.min(256 - start);
                self.print_palette(start, count);
            }
            "tiles" | "tile" => {
                let tile = optional_count(&mut parts, 0)?.min(1023);
                let count = optional_count(&mut parts, 4)?.min(64);
                let bpp = optional_count(&mut parts, 4)?;
                if bpp != 4 && bpp != 8 {
                    bail!("tile depth must be 4 or 8");
                }
                self.print_tiles(tile, count, bpp);
            }
            "oam" | "sprites" => {
                let count = optional_count(&mut parts, 16)?.min(128);
                self.print_oam(count);
            }
            "bg" | "bgs" | "backgrounds" => self.print_backgrounds(),
            "layers" => self.print_layers(),
            "layer" => {
                let name = parts
                    .next()
                    .context("usage: layer <bg0..bg3|obj|win0|win1|objwin> <on|off>")?;
                let state = parts.next().context("usage: layer <name> <on|off>")?;
                let enabled = match state {
                    "on" | "1" | "true" => true,
                    "off" | "0" | "false" => false,
                    _ => bail!("layer state must be on or off"),
                };
                self.set_layer(name, enabled)?;
            }
            "ppu" => self.print_ppu(),
            "dma" => self.print_dma(),
            "timers" => self.print_timers(),
            "irq" => self.print_irq(),
            "reset" => {
                self.system.reset();
                self.steps = 0;
                self.rewind.clear();
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
        println!("  w8/w16/w32 <addr> <v>   write memory");
        println!("  ss/sl [slot]            save/load an in-memory state slot");
        println!("  states                  list state slots and rewind buffer");
        println!("  record on|off|clear     control rewind recording");
        println!("  rewind [n]              restore n recorded frames");
        println!("  palette [start] [count] palette viewer");
        println!("  tiles [tile] [count] [4|8] tile viewer");
        println!("  oam [count]             sprite viewer");
        println!("  bg                      background viewer");
        println!("  layers, layer <name> on|off");
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
            self.record_snapshot();
            let _ = self.system.run_frame();
        }
    }

    fn continue_run(&mut self) {
        let mut skipped = Some(self.system.cpu.pc());
        let start_steps = self.steps;
        let mut last_frame = self.system.bus.ppu.frame_count;
        let mut hit_breakpoint = None;
        self.record_snapshot();

        for _ in 0..CONTINUE_STEP_LIMIT {
            let pc = self.system.cpu.pc();
            if self.breakpoints.contains(&pc) && skipped != Some(pc) {
                hit_breakpoint = Some(pc);
                break;
            }
            skipped = None;
            self.system.step();
            self.steps += 1;
            if self.system.bus.ppu.frame_count != last_frame {
                last_frame = self.system.bus.ppu.frame_count;
                self.record_snapshot();
            }
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

    fn load_slot(&mut self, slot: u8) -> Result<()> {
        let state = self
            .save_slots
            .get(&slot)
            .cloned()
            .with_context(|| format!("no save state in slot {slot}"))?;
        self.system.load_state(&state).context("load state")?;
        self.rewind.clear();
        println!("loaded state {slot}");
        self.print_state();
        Ok(())
    }

    fn print_states(&self) {
        if self.save_slots.is_empty() {
            println!("no save-state slots");
        } else {
            for (slot, state) in &self.save_slots {
                println!("slot {slot}: {} bytes", state.len());
            }
        }
        println!(
            "rewind: {} snapshots (limit {}, recording {})",
            self.rewind.len(),
            self.rewind_limit,
            if self.recording { "on" } else { "off" }
        );
    }

    fn record_command(&mut self, argument: Option<&str>) -> Result<()> {
        match argument.unwrap_or("status") {
            "on" => {
                self.recording = true;
                println!("rewind recording on ({} frames)", self.rewind_limit);
            }
            "off" => {
                self.recording = false;
                println!("rewind recording off");
            }
            "clear" => {
                self.rewind.clear();
                println!("rewind buffer cleared");
            }
            "status" => self.print_states(),
            other => bail!("unknown record option \"{other}\"; use on, off, clear, or status"),
        }
        Ok(())
    }

    fn record_snapshot(&mut self) {
        if !self.recording {
            return;
        }
        if let Ok(state) = self.system.save_state() {
            if self.rewind.len() >= self.rewind_limit {
                self.rewind.pop_front();
            }
            self.rewind.push_back(state);
        }
    }

    fn rewind_frames(&mut self, count: usize) -> Result<()> {
        let mut restored = 0;
        for _ in 0..count {
            let Some(state) = self.rewind.pop_back() else {
                break;
            };
            self.system.load_state(&state).context("rewind")?;
            restored += 1;
        }
        if restored == 0 {
            println!("rewind buffer is empty");
        } else {
            println!("rewound {restored} frame(s)");
        }
        Ok(())
    }

    fn print_palette(&self, start: u32, count: u32) {
        for index in start..start + count {
            let color = self.system.bus.ppu.read_palette16(0x0500_0000 + index * 2);
            let red = color & 0x1F;
            let green = (color >> 5) & 0x1F;
            let blue = (color >> 10) & 0x1F;
            println!(
                "{index:03}  {color:04X}  #{:02X}{:02X}{:02X}",
                expand5(red),
                expand5(green),
                expand5(blue)
            );
        }
    }

    fn print_tiles(&self, first: u32, count: u32, bpp: u32) {
        let tile_size = if bpp == 8 { 64 } else { 32 };
        for tile in first..first + count {
            let base = 0x0600_0000u32.wrapping_add(tile.wrapping_mul(tile_size));
            println!("tile {tile:03} @ 0x{base:08X}");
            for row in 0..8 {
                print!("  ");
                for column in 0..8 {
                    let index = if bpp == 8 {
                        self.system.peek8(base + row * 8 + column)
                    } else {
                        let byte = self.system.peek8(base + row * 4 + column / 2);
                        if column & 1 == 0 {
                            byte & 0x0F
                        } else {
                            byte >> 4
                        }
                    };
                    if bpp == 8 {
                        print!("{index:02X}");
                    } else {
                        print!("{index:X}");
                    }
                }
                println!();
            }
        }
    }

    fn print_oam(&self, count: u32) {
        for index in 0..count {
            let base = 0x0700_0000 + index * 8;
            let attr0 = self.system.peek16(base);
            let attr1 = self.system.peek16(base + 2);
            let attr2 = self.system.peek16(base + 4);
            let affine = attr0 & 0x0100 != 0;
            let flag = attr0 & 0x0200 != 0;
            let mode = (attr0 >> 10) & 3;
            let shape = (attr0 >> 14) & 3;
            let size = (attr1 >> 14) & 3;
            let (width, height) = sprite_dimensions(shape, size);
            let x = attr1 & 0x01FF;
            let y = attr0 & 0x00FF;
            let tile = attr2 & 0x03FF;
            let priority = (attr2 >> 10) & 3;
            let palette = (attr2 >> 12) & 0xF;
            let eight_bpp = attr0 & 0x2000 != 0;
            let flip = if affine {
                format!("affine={}", (attr1 >> 9) & 0x1F)
            } else {
                format!(
                    "{}{}",
                    if attr1 & 0x1000 != 0 { "hflip " } else { "" },
                    if attr1 & 0x2000 != 0 { "vflip" } else { "" }
                )
            };
            let flag = if affine {
                if flag { "double" } else { "affine" }
            } else if flag {
                "disabled"
            } else {
                "normal"
            };
            println!(
                "{index:03} x={x:3} y={y:3} {width}x{height} tile={tile:03} prio={priority} pal={palette} mode={mode} {}bpp {flag} {flip}",
                if eight_bpp { 8 } else { 4 }
            );
        }
    }

    fn print_backgrounds(&self) {
        let ppu = &self.system.bus.ppu;
        println!("DISPCNT={:04X} mode={}", ppu.dispcnt, ppu.dispcnt & 7);
        for index in 0..4 {
            let control = ppu.bgcnt[index];
            let enabled = ppu.dispcnt & (0x0100 << index) != 0;
            let priority = control & 3;
            let char_base = u32::from((control >> 2) & 3) * 0x4000;
            let screen_base = u32::from((control >> 8) & 0x1F) * 0x800;
            let size = (control >> 14) & 3;
            let mosaic = control & 0x0040 != 0;
            let eight_bpp = control & 0x0080 != 0;
            let affine = control & 0x2000 != 0;
            println!(
                "BG{index}: on={enabled} prio={priority} char=0x{:08X} screen=0x{:08X} size={size} bpp={} affine={affine} mosaic={mosaic} hofs={} vofs={}",
                0x0600_0000 + char_base,
                0x0600_0000 + screen_base,
                if eight_bpp { 8 } else { 4 },
                ppu.bghofs[index],
                ppu.bgvofs[index]
            );
            if affine {
                println!(
                    "     pa={} pb={} pc={} pd={} x={} y={}",
                    ppu.bgpa[index.min(1)],
                    ppu.bgpb[index.min(1)],
                    ppu.bgpc[index.min(1)],
                    ppu.bgpd[index.min(1)],
                    ppu.bgx[index.min(1)],
                    ppu.bgy[index.min(1)]
                );
            }
        }
    }

    fn print_layers(&self) {
        let dispcnt = self.system.peek16(0x0400_0000);
        println!("DISPCNT={dispcnt:04X}");
        for (name, bit) in LAYER_BITS {
            println!(
                "  {name:6}: {}",
                if dispcnt & bit != 0 { "on" } else { "off" }
            );
        }
    }

    fn set_layer(&mut self, name: &str, enabled: bool) -> Result<()> {
        let bit = layer_bit(name).with_context(|| format!("unknown layer \"{name}\""))?;
        let mut dispcnt = self.system.peek16(0x0400_0000);
        if enabled {
            dispcnt |= bit;
        } else {
            dispcnt &= !bit;
        }
        self.system.poke16(0x0400_0000, dispcnt);
        println!("{name} {}", if enabled { "on" } else { "off" });
        self.print_layers();
        Ok(())
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

const LAYER_BITS: [(&str, u16); 8] = [
    ("bg0", 0x0100),
    ("bg1", 0x0200),
    ("bg2", 0x0400),
    ("bg3", 0x0800),
    ("obj", 0x1000),
    ("win0", 0x2000),
    ("win1", 0x4000),
    ("objwin", 0x8000),
];

fn layer_bit(name: &str) -> Option<u16> {
    LAYER_BITS
        .iter()
        .find_map(|(layer, bit)| (*layer == name).then_some(*bit))
}

fn expand5(value: impl Into<u32>) -> u8 {
    let value = value.into();
    ((value << 3) | (value >> 2)) as u8
}

fn sprite_dimensions(shape: u16, size: u16) -> (u32, u32) {
    const SQUARE: [(u32, u32); 4] = [(8, 8), (16, 16), (32, 32), (64, 64)];
    const WIDE: [(u32, u32); 4] = [(16, 8), (32, 8), (32, 16), (64, 32)];
    const TALL: [(u32, u32); 4] = [(8, 16), (8, 32), (16, 32), (32, 64)];
    match shape {
        0 => SQUARE[size as usize & 3],
        1 => WIDE[size as usize & 3],
        2 => TALL[size as usize & 3],
        _ => (0, 0),
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

fn optional_slot(parts: &mut SplitWhitespace) -> Result<u8> {
    match parts.next() {
        Some(value) => value
            .parse()
            .with_context(|| format!("invalid slot \"{value}\"")),
        None => Ok(0),
    }
}

fn required_value(parts: &mut SplitWhitespace) -> Result<u32> {
    match parts.next() {
        Some(value) => parse_address(value),
        None => bail!("missing value"),
    }
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
    fn save_slots_restore_execution() {
        let mut debugger = debugger();
        for _ in 0..3 {
            let _ = debugger.system.run_frame();
        }
        let frame = debugger.system.bus.ppu.frame_count;
        debugger.command("ss 0").unwrap();
        for _ in 0..3 {
            let _ = debugger.system.run_frame();
        }
        assert_ne!(debugger.system.bus.ppu.frame_count, frame);
        debugger.command("sl 0").unwrap();
        assert_eq!(debugger.system.bus.ppu.frame_count, frame);
    }

    #[test]
    fn memory_editor_writes_ram() {
        let mut debugger = debugger();
        debugger.command("w32 0x02000000 0x12345678").unwrap();
        assert_eq!(debugger.system.peek32(0x0200_0000), 0x1234_5678);
    }

    #[test]
    fn layer_toggle_updates_dispcnt() {
        let mut debugger = debugger();
        debugger.command("layer obj on").unwrap();
        assert_ne!(debugger.system.bus.ppu.dispcnt & 0x1000, 0);
        debugger.command("layer obj off").unwrap();
        assert_eq!(debugger.system.bus.ppu.dispcnt & 0x1000, 0);
    }

    #[test]
    fn a_frame_advances_the_ppu() {
        let mut debugger = debugger();
        let before = debugger.system.bus.ppu.frame_count;
        debugger.run_frames(1);
        assert!(debugger.system.bus.ppu.frame_count > before);
    }
}
