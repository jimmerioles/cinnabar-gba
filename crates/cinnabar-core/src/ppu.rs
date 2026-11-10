//! GBA Picture Processing Unit.
//!
//! The PPU renders one scanline at a time. Backgrounds, sprites, windows,
//! mosaic and color effects are composed into a BGR555 framebuffer that the
//! frontend converts to RGB.

use serde::{Deserialize, Serialize};

use crate::irq::{self, Irq};
use crate::{CYCLES_PER_SCANLINE, SCANLINES, SCREEN_HEIGHT, SCREEN_WIDTH, VISIBLE_SCANLINES};

#[derive(Clone, Copy, Debug, Default)]
pub struct PpuEvents {
    pub hblank: bool,
    pub vblank: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Layer {
    Obj,
    Bg0,
    Bg1,
    Bg2,
    Bg3,
    Backdrop,
}

impl Layer {
    fn index(self) -> u8 {
        match self {
            Self::Bg0 => 0,
            Self::Bg1 => 1,
            Self::Bg2 => 2,
            Self::Bg3 => 3,
            Self::Obj => 4,
            Self::Backdrop => 5,
        }
    }

    fn rank(self) -> u8 {
        match self {
            Self::Obj => 0,
            Self::Bg0 => 1,
            Self::Bg1 => 2,
            Self::Bg2 => 3,
            Self::Bg3 => 4,
            Self::Backdrop => 5,
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct Candidate {
    color: u16,
    priority: u8,
    layer: Layer,
    semi: bool,
    /// OAM index for sprites; 0 for backgrounds. Used only to break ties.
    order: u16,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Ppu {
    pub vcount: u16,
    pub dispcnt: u16,
    pub dispstat: u16,
    pub bgcnt: [u16; 4],
    pub bghofs: [u16; 4],
    pub bgvofs: [u16; 4],
    pub bgpa: [i16; 2],
    pub bgpb: [i16; 2],
    pub bgpc: [i16; 2],
    pub bgpd: [i16; 2],
    pub bgx: [i32; 2],
    pub bgy: [i32; 2],
    pub win0h: u16,
    pub win1h: u16,
    pub win0v: u16,
    pub win1v: u16,
    pub winin: u16,
    pub winout: u16,
    pub mosaic: u16,
    pub bldcnt: u16,
    pub bldalpha: u16,
    pub bldy: u16,

    pub vram: Vec<u8>,
    pub palette: Vec<u8>,
    pub oam: Vec<u8>,
    pub framebuffer: Vec<u16>,

    line_cycles: u32,
    pub frame_ready: bool,
    pub frame_count: u64,
    /// Incremented every time VBlank begins. Used by `VBlankIntrWait` so the
    /// wait does not depend on the game having enabled the VBlank IRQ.
    pub vblank_count: u64,
}

impl Default for Ppu {
    fn default() -> Self {
        Self::new()
    }
}

impl Ppu {
    pub fn new() -> Self {
        Self {
            vcount: 0,
            dispcnt: 0,
            dispstat: 0,
            bgcnt: [0; 4],
            bghofs: [0; 4],
            bgvofs: [0; 4],
            bgpa: [0; 2],
            bgpb: [0; 2],
            bgpc: [0; 2],
            bgpd: [0; 2],
            bgx: [0; 2],
            bgy: [0; 2],
            win0h: 0,
            win1h: 0,
            win0v: 0,
            win1v: 0,
            winin: 0,
            winout: 0,
            mosaic: 0,
            bldcnt: 0,
            bldalpha: 0,
            bldy: 0,
            vram: vec![0; 0x1_8000],
            palette: vec![0; 0x400],
            oam: vec![0; 0x400],
            framebuffer: vec![0; SCREEN_WIDTH * SCREEN_HEIGHT],
            line_cycles: 0,
            frame_ready: false,
            frame_count: 0,
            vblank_count: 0,
        }
    }

    pub fn line_cycles(&self) -> u32 {
        self.line_cycles
    }

    pub fn step(&mut self, cycles: u32, irq: &mut Irq) -> PpuEvents {
        let mut events = PpuEvents::default();
        self.line_cycles += cycles;
        while self.line_cycles >= CYCLES_PER_SCANLINE {
            self.line_cycles -= CYCLES_PER_SCANLINE;
            let line = self.vcount;

            self.dispstat &= !0x0002;

            if line < VISIBLE_SCANLINES {
                self.render_line(line);
                self.dispstat |= 0x0002;
                if self.dispstat & 0x0010 != 0 {
                    irq.request(irq::bits::HBLANK);
                }
                events.hblank = true;
            }

            self.vcount = line + 1;
            if self.vcount == VISIBLE_SCANLINES {
                self.dispstat |= 0x0001;
                self.vblank_count = self.vblank_count.wrapping_add(1);
                if self.dispstat & 0x0008 != 0 {
                    irq.request(irq::bits::VBLANK);
                }
                events.vblank = true;
            }

            if self.vcount >= SCANLINES {
                self.vcount = 0;
                self.dispstat &= !0x0001;
                self.frame_ready = true;
                self.frame_count = self.frame_count.wrapping_add(1);
            }

            let target = self.dispstat >> 8;
            if self.vcount == target {
                self.dispstat |= 0x0004;
                if self.dispstat & 0x0020 != 0 {
                    irq.request(irq::bits::VCOUNT);
                }
            } else {
                self.dispstat &= !0x0004;
            }
        }
        events
    }

    #[allow(clippy::needless_range_loop)] // pixel math reads better with x
    fn render_line(&mut self, y: u16) {
        let base = y as usize * SCREEN_WIDTH;
        if self.dispcnt & 0x0080 != 0 || self.dispcnt & 0x0007 > 5 {
            for x in 0..SCREEN_WIDTH {
                self.framebuffer[base + x] = 0x7FFF;
            }
            return;
        }

        let mode = self.dispcnt & 7;
        let backdrop = Candidate {
            color: self.palette16(0),
            priority: 3,
            layer: Layer::Backdrop,
            semi: false,
            order: 0,
        };
        let sprites: [Option<Candidate>; SCREEN_WIDTH] = if self.dispcnt & 0x1000 != 0 {
            self.sprite_line(y as usize)
        } else {
            [None; SCREEN_WIDTH]
        };
        let obj_window: [bool; SCREEN_WIDTH] = if self.dispcnt & 0x8000 != 0 {
            self.obj_window_line(y as usize)
        } else {
            [false; SCREEN_WIDTH]
        };

        for x in 0..SCREEN_WIDTH {
            let window = self.window_mask(x, y as usize, obj_window[x]);
            let mut best: Option<Candidate> = None;
            let mut second: Option<Candidate> = None;

            if mode >= 3 {
                if let Some(candidate) = self.bitmap_pixel(x, y as usize, mode) {
                    insert_candidate(&mut best, &mut second, candidate);
                }
            } else {
                for layer in 0..4usize {
                    if !self.bg_enabled(layer, mode) {
                        continue;
                    }
                    if window & (1 << layer) == 0 {
                        continue;
                    }
                    if let Some(candidate) = self.bg_pixel(layer, x, y as usize, mode) {
                        insert_candidate(&mut best, &mut second, candidate);
                    }
                }
            }

            if window & 0x10 != 0 {
                if let Some(candidate) = sprites[x] {
                    insert_candidate(&mut best, &mut second, candidate);
                }
            }

            let top = best.unwrap_or(backdrop);
            let below = second.unwrap_or(backdrop);
            let color = self.apply_effects(top, below, window);
            self.framebuffer[base + x] = color;
        }
    }

    fn apply_effects(&self, top: Candidate, below: Candidate, window: u16) -> u16 {
        let effect = (self.bldcnt >> 6) & 3;
        let first = (self.bldcnt & 0x3F) as u8;
        let second = ((self.bldcnt >> 8) & 0x3F) as u8;
        let top_bit = 1u8 << top.layer.index();
        let below_bit = 1u8 << below.layer.index();

        if effect == 0 || window & 0x20 == 0 || first & top_bit == 0 {
            return top.color;
        }

        match effect {
            1 => {
                if top.semi {
                    blend_alpha(top.color, below.color, 8, 8)
                } else if second & below_bit != 0 {
                    let eva = (self.bldalpha & 0x1F) as u32;
                    let evb = ((self.bldalpha >> 8) & 0x1F) as u32;
                    blend_alpha(top.color, below.color, eva, evb)
                } else {
                    top.color
                }
            }
            2 => brightness_up(top.color, (self.bldy & 0x1F) as u32),
            3 => brightness_down(top.color, (self.bldy & 0x1F) as u32),
            _ => top.color,
        }
    }

    fn bg_enabled(&self, layer: usize, mode: u16) -> bool {
        if self.dispcnt & (1 << (8 + layer)) == 0 {
            return false;
        }
        match mode {
            0 => true,
            1 => layer <= 2,
            2 => layer >= 2,
            _ => false,
        }
    }

    fn bg_pixel(&self, layer: usize, x: usize, y: usize, mode: u16) -> Option<Candidate> {
        if mode == 1 && layer == 2 || mode == 2 && layer >= 2 {
            self.bg_affine_pixel(layer, x, y)
        } else {
            self.bg_text_pixel(layer, x, y)
        }
    }

    fn bg_text_pixel(&self, layer: usize, x: usize, y: usize) -> Option<Candidate> {
        let cnt = self.bgcnt[layer];
        let char_base = ((cnt >> 2) & 3) as usize * 0x4000;
        let screen_base = ((cnt >> 8) & 0x1F) as usize * 0x800;
        let size = (cnt >> 14) & 3;
        let width_tiles = if size & 1 != 0 { 64 } else { 32 };
        let height_tiles = if size & 2 != 0 { 64 } else { 32 };
        let (mosaic_x, mosaic_y) = if cnt & 0x40 != 0 {
            mosaic_dimensions(self.mosaic)
        } else {
            (1, 1)
        };
        let sample_x = mosaic_coord(x, mosaic_x);
        let sample_y = mosaic_coord(y, mosaic_y);
        let width = width_tiles * 8;
        let height = height_tiles * 8;

        let sx = (sample_x + self.bghofs[layer] as usize) % width;
        let sy = (sample_y + self.bgvofs[layer] as usize) % height;
        let map_x = sx / 8;
        let map_y = sy / 8;

        let block = (map_y / 32) * (width_tiles / 32) + (map_x / 32);
        let entry_addr = screen_base + block * 0x800 + ((map_y % 32) * 32 + (map_x % 32)) * 2;
        let entry = self.vram16(entry_addr);

        let tile = (entry & 0x3FF) as usize;
        let mut tx = sx % 8;
        let mut ty = sy % 8;
        if entry & 0x0400 != 0 {
            tx = 7 - tx;
        }
        if entry & 0x0800 != 0 {
            ty = 7 - ty;
        }
        let palette_bank = ((entry >> 12) & 0xF) as usize;

        let (color_index, palette_index) = if cnt & 0x80 != 0 {
            let index = self.vram8(char_base + tile * 64 + ty * 8 + tx) as usize;
            (index, index)
        } else {
            let byte = self.vram8(char_base + tile * 32 + ty * 4 + tx / 2);
            let index = ((byte >> ((tx & 1) * 4)) & 0xF) as usize;
            (index, palette_bank * 16 + index)
        };
        if color_index == 0 {
            return None;
        }
        Some(Candidate {
            color: self.palette16(palette_index),
            priority: (cnt & 3) as u8,
            layer: match layer {
                0 => Layer::Bg0,
                1 => Layer::Bg1,
                2 => Layer::Bg2,
                _ => Layer::Bg3,
            },
            semi: false,
            order: 0,
        })
    }

    fn bg_affine_pixel(&self, layer: usize, x: usize, y: usize) -> Option<Candidate> {
        let idx = layer - 2;
        let cnt = self.bgcnt[layer];
        let char_base = ((cnt >> 2) & 3) as usize * 0x4000;
        let screen_base = ((cnt >> 8) & 0x1F) as usize * 0x800;
        let size = (cnt >> 14) & 3;
        let dimension = 128usize << size;

        let pa = self.bgpa[idx] as i32;
        let pb = self.bgpb[idx] as i32;
        let pc = self.bgpc[idx] as i32;
        let pd = self.bgpd[idx] as i32;
        let dx = x as i32 - 120;
        let dy = y as i32 - 80;

        let mut tex_x = (self.bgx[idx]
            .wrapping_add(pa.wrapping_mul(dx))
            .wrapping_add(pb.wrapping_mul(dy)))
            >> 8;
        let mut tex_y = (self.bgy[idx]
            .wrapping_add(pc.wrapping_mul(dx))
            .wrapping_add(pd.wrapping_mul(dy)))
            >> 8;

        if cnt & 0x2000 != 0 {
            tex_x = tex_x.rem_euclid(dimension as i32);
            tex_y = tex_y.rem_euclid(dimension as i32);
        } else if tex_x < 0 || tex_y < 0 || tex_x >= dimension as i32 || tex_y >= dimension as i32 {
            return None;
        }

        let map_width = dimension / 8;
        let map_addr = screen_base + (tex_y as usize / 8) * map_width + (tex_x as usize / 8);
        let tile = self.vram8(map_addr) as usize;
        let index = self
            .vram8(char_base + tile * 64 + (tex_y as usize % 8) * 8 + (tex_x as usize % 8))
            as usize;
        if index == 0 {
            return None;
        }
        Some(Candidate {
            color: self.palette16(index),
            priority: (cnt & 3) as u8,
            layer: if layer == 2 { Layer::Bg2 } else { Layer::Bg3 },
            semi: false,
            order: 0,
        })
    }

    fn bitmap_pixel(&self, x: usize, y: usize, mode: u16) -> Option<Candidate> {
        let color = match mode {
            3 => self.vram16((y * SCREEN_WIDTH + x) * 2),
            4 => {
                let page = if self.dispcnt & 0x10 != 0 { 0xA000 } else { 0 };
                self.palette16(self.vram8(page + y * SCREEN_WIDTH + x) as usize)
            }
            5 => {
                if x >= 160 || y >= 128 {
                    return None;
                }
                let page = if self.dispcnt & 0x10 != 0 { 0xA000 } else { 0 };
                self.vram16(page + (y * 160 + x) * 2)
            }
            _ => return None,
        };
        Some(Candidate {
            color,
            priority: 0,
            layer: Layer::Bg2,
            semi: false,
            order: 0,
        })
    }

    #[allow(clippy::needless_range_loop)] // x drives both the line and texture
    fn sprite_line(&self, y: usize) -> [Option<Candidate>; SCREEN_WIDTH] {
        let mut line: [Option<Candidate>; SCREEN_WIDTH] = [None; SCREEN_WIDTH];

        for index in 0..128 {
            let base = index * 8;
            let attr0 = self.oam16(base);
            let attr1 = self.oam16(base + 2);
            let attr2 = self.oam16(base + 4);

            let affine = attr0 & 0x0100 != 0;
            if !affine && attr0 & 0x0200 != 0 {
                continue;
            }
            let mode = (attr0 >> 10) & 3;
            if mode >= 2 {
                continue;
            }

            let shape = (attr0 >> 14) & 3;
            let size = (attr1 >> 14) & 3;
            let Some((w, h)) = sprite_size(shape, size) else {
                continue;
            };

            let sx = (attr1 & 0x1FF) as i32;
            let sy = (attr0 & 0xFF) as i32;
            let double_size = affine && attr0 & 0x0200 != 0;
            let bw = if double_size { w * 2 } else { w };
            let bh = if double_size { h * 2 } else { h };

            if (y as i32) < sy || (y as i32) >= sy + bh as i32 {
                continue;
            }
            let x_start = sx.clamp(0, SCREEN_WIDTH as i32) as usize;
            let x_end = (sx + bw as i32).clamp(0, SCREEN_WIDTH as i32) as usize;
            if x_start >= x_end {
                continue;
            }

            let (pa, pb, pc, pd) = if affine {
                let affine_index = ((attr1 >> 9) & 0x1F) as usize;
                let param = affine_index * 32 + 6;
                (
                    self.oam_i16(param) as i32,
                    self.oam_i16(param + 8) as i32,
                    self.oam_i16(param + 16) as i32,
                    self.oam_i16(param + 24) as i32,
                )
            } else {
                (0, 0, 0, 0)
            };

            let (mosaic_x, mosaic_y) = if attr0 & 0x1000 != 0 {
                object_mosaic_dimensions(self.mosaic)
            } else {
                (1, 1)
            };
            let tile = (attr2 & 0x3FF) as usize;
            let one_d = self.dispcnt & 0x0040 != 0;
            let eight_bpp = attr0 & 0x2000 != 0;
            let palette_bank = ((attr2 >> 12) & 0xF) as usize;
            let priority = ((attr2 >> 10) & 3) as u8;
            let semi = mode == 1;

            for x in x_start..x_end {
                let (tx, ty) = if affine {
                    let sample_x = mosaic_coord(x, mosaic_x);
                    let sample_y = mosaic_coord(y, mosaic_y);
                    let dx = sample_x as i32 - (sx + bw as i32 / 2);
                    let dy = sample_y as i32 - (sy + bh as i32 / 2);
                    let tx = ((pa * dx + pb * dy) >> 8) + w as i32 / 2;
                    let ty = ((pc * dx + pd * dy) >> 8) + h as i32 / 2;
                    if tx < 0 || ty < 0 || tx >= w as i32 || ty >= h as i32 {
                        continue;
                    }
                    (tx as usize, ty as usize)
                } else {
                    let sample_x = mosaic_coord(x, mosaic_x)
                        .max(sx as usize)
                        .min((sx + w as i32 - 1) as usize);
                    let sample_y = mosaic_coord(y, mosaic_y)
                        .max(sy as usize)
                        .min((sy + h as i32 - 1) as usize);
                    let mut tx = sample_x as i32 - sx;
                    let mut ty = sample_y as i32 - sy;
                    if attr1 & 0x1000 != 0 {
                        tx = w as i32 - 1 - tx;
                    }
                    if attr1 & 0x2000 != 0 {
                        ty = h as i32 - 1 - ty;
                    }
                    (tx as usize, ty as usize)
                };

                let tile_x = tx / 8;
                let tile_y = ty / 8;
                let tile_num = if one_d {
                    tile + tile_y * (w / 8) + tile_x
                } else {
                    tile + tile_y * 32 + tile_x
                };

                let (color_index, palette_index) = if eight_bpp {
                    let index =
                        self.vram8(0x1_0000 + tile_num * 64 + (ty % 8) * 8 + (tx % 8)) as usize;
                    (index, index)
                } else {
                    let byte = self.vram8(0x1_0000 + tile_num * 32 + (ty % 8) * 4 + (tx % 8) / 2);
                    let index = ((byte >> (((tx % 8) & 1) * 4)) & 0xF) as usize;
                    (index, palette_bank * 16 + index)
                };
                if color_index == 0 {
                    continue;
                }

                let candidate = Candidate {
                    color: self.palette16(palette_index),
                    priority,
                    layer: Layer::Obj,
                    semi,
                    order: index as u16,
                };
                let replace = match line[x] {
                    None => true,
                    Some(current) => (priority, index as u16) < (current.priority, current.order),
                };
                if replace {
                    line[x] = Some(candidate);
                }
            }
        }

        line
    }

    fn window_mask(&self, x: usize, y: usize, obj_window: bool) -> u16 {
        if self.dispcnt & 0xE000 == 0 {
            return 0x3F;
        }

        let in_win0 = self.dispcnt & 0x2000 != 0 && self.in_window(self.win0h, self.win0v, x, y);
        if in_win0 {
            return self.winin & 0x3F;
        }
        let in_win1 = self.dispcnt & 0x4000 != 0 && self.in_window(self.win1h, self.win1v, x, y);
        if in_win1 {
            return (self.winin >> 8) & 0x3F;
        }
        if self.dispcnt & 0x8000 != 0 && obj_window {
            return (self.winout >> 8) & 0x3F;
        }
        self.winout & 0x3F
    }

    #[allow(clippy::needless_range_loop)] // x drives the sprite texture
    fn obj_window_line(&self, y: usize) -> [bool; SCREEN_WIDTH] {
        let mut line = [false; SCREEN_WIDTH];

        for index in 0..128 {
            let base = index * 8;
            let attr0 = self.oam16(base);
            let attr1 = self.oam16(base + 2);
            let attr2 = self.oam16(base + 4);

            if (attr0 >> 10) & 3 != 2 {
                continue;
            }
            let affine = attr0 & 0x0100 != 0;
            let shape = (attr0 >> 14) & 3;
            let size = (attr1 >> 14) & 3;
            let Some((w, h)) = sprite_size(shape, size) else {
                continue;
            };
            let sx = (attr1 & 0x1FF) as i32;
            let sy = (attr0 & 0xFF) as i32;
            let double_size = affine && attr0 & 0x0200 != 0;
            let bw = if double_size { w * 2 } else { w };
            let bh = if double_size { h * 2 } else { h };
            if (y as i32) < sy || (y as i32) >= sy + bh as i32 {
                continue;
            }
            let x_start = sx.clamp(0, SCREEN_WIDTH as i32) as usize;
            let x_end = (sx + bw as i32).clamp(0, SCREEN_WIDTH as i32) as usize;
            if x_start >= x_end {
                continue;
            }

            let (pa, pb, pc, pd) = if affine {
                let affine_index = ((attr1 >> 9) & 0x1F) as usize;
                let param = affine_index * 32 + 6;
                (
                    self.oam_i16(param) as i32,
                    self.oam_i16(param + 8) as i32,
                    self.oam_i16(param + 16) as i32,
                    self.oam_i16(param + 24) as i32,
                )
            } else {
                (0, 0, 0, 0)
            };

            let (mosaic_x, mosaic_y) = if attr0 & 0x1000 != 0 {
                object_mosaic_dimensions(self.mosaic)
            } else {
                (1, 1)
            };
            let tile = (attr2 & 0x3FF) as usize;
            let one_d = self.dispcnt & 0x0040 != 0;
            let eight_bpp = attr0 & 0x2000 != 0;

            for x in x_start..x_end {
                let (tx, ty) = if affine {
                    let sample_x = mosaic_coord(x, mosaic_x);
                    let sample_y = mosaic_coord(y, mosaic_y);
                    let dx = sample_x as i32 - (sx + bw as i32 / 2);
                    let dy = sample_y as i32 - (sy + bh as i32 / 2);
                    let tx = ((pa * dx + pb * dy) >> 8) + w as i32 / 2;
                    let ty = ((pc * dx + pd * dy) >> 8) + h as i32 / 2;
                    if tx < 0 || ty < 0 || tx >= w as i32 || ty >= h as i32 {
                        continue;
                    }
                    (tx as usize, ty as usize)
                } else {
                    let sample_x = mosaic_coord(x, mosaic_x)
                        .max(sx as usize)
                        .min((sx + w as i32 - 1) as usize);
                    let sample_y = mosaic_coord(y, mosaic_y)
                        .max(sy as usize)
                        .min((sy + h as i32 - 1) as usize);
                    let mut tx = sample_x as i32 - sx;
                    let mut ty = sample_y as i32 - sy;
                    if attr1 & 0x1000 != 0 {
                        tx = w as i32 - 1 - tx;
                    }
                    if attr1 & 0x2000 != 0 {
                        ty = h as i32 - 1 - ty;
                    }
                    (tx as usize, ty as usize)
                };

                let tile_x = tx / 8;
                let tile_y = ty / 8;
                let tile_num = if one_d {
                    tile + tile_y * (w / 8) + tile_x
                } else {
                    tile + tile_y * 32 + tile_x
                };
                let color_index = if eight_bpp {
                    self.vram8(0x1_0000 + tile_num * 64 + (ty % 8) * 8 + (tx % 8)) as usize
                } else {
                    let byte = self.vram8(0x1_0000 + tile_num * 32 + (ty % 8) * 4 + (tx % 8) / 2);
                    ((byte >> (((tx % 8) & 1) * 4)) & 0xF) as usize
                };
                if color_index != 0 {
                    line[x] = true;
                }
            }
        }

        line
    }

    fn in_window(&self, h: u16, v: u16, x: usize, y: usize) -> bool {
        let x1 = (h & 0xFF) as usize;
        let x2 = (h >> 8) as usize;
        let y1 = (v & 0xFF) as usize;
        let y2 = (v >> 8) as usize;
        x >= x1 && x < x2 && y >= y1 && y < y2
    }

    /// VRAM is 96 KiB inside a 128 KiB window. The upper 32 KiB mirrors the
    /// first 32 KiB, and the whole window repeats every 128 KiB.
    fn vram_offset(addr: usize) -> usize {
        let offset = addr % 0x2_0000;
        if offset >= 0x1_8000 {
            offset - 0x1_8000
        } else {
            offset
        }
    }

    pub fn read_vram8(&self, addr: u32) -> u8 {
        self.vram8(Self::vram_offset(addr as usize))
    }

    pub fn read_vram16(&self, addr: u32) -> u16 {
        self.vram16(Self::vram_offset(addr as usize))
    }

    pub fn read_vram32(&self, addr: u32) -> u32 {
        let offset = Self::vram_offset(addr as usize);
        self.vram16(offset) as u32 | ((self.vram16(offset + 2) as u32) << 16)
    }

    pub fn write_vram8(&mut self, addr: u32, value: u8) {
        // A byte store to VRAM is duplicated into both bytes of the
        // addressed halfword, matching the GBA's 16-bit video memory.
        let len = self.vram.len();
        let offset = Self::vram_offset((addr as usize) & !1);
        self.vram[offset] = value;
        self.vram[(offset + 1) % len] = value;
    }

    pub fn write_vram16(&mut self, addr: u32, value: u16) {
        let len = self.vram.len();
        let offset = Self::vram_offset(addr as usize);
        let bytes = value.to_le_bytes();
        self.vram[offset] = bytes[0];
        self.vram[(offset + 1) % len] = bytes[1];
    }

    pub fn write_vram32(&mut self, addr: u32, value: u32) {
        self.write_vram16(addr, value as u16);
        self.write_vram16(addr.wrapping_add(2), (value >> 16) as u16);
    }

    pub fn read_palette8(&self, addr: u32) -> u8 {
        self.palette[(addr as usize) % self.palette.len()]
    }

    pub fn read_palette16(&self, addr: u32) -> u16 {
        let offset = (addr as usize) % self.palette.len();
        u16::from_le_bytes([
            self.palette[offset],
            self.palette[(offset + 1) % self.palette.len()],
        ])
    }

    pub fn write_palette8(&mut self, addr: u32, value: u8) {
        // Palette byte stores are duplicated across the halfword too.
        let len = self.palette.len();
        let offset = (addr as usize & !1) % len;
        self.palette[offset] = value;
        self.palette[(offset + 1) % len] = value;
    }

    pub fn write_palette16(&mut self, addr: u32, value: u16) {
        let len = self.palette.len();
        let offset = (addr as usize) % len;
        let bytes = value.to_le_bytes();
        self.palette[offset] = bytes[0];
        self.palette[(offset + 1) % len] = bytes[1];
    }

    pub fn write_palette32(&mut self, addr: u32, value: u32) {
        self.write_palette16(addr, value as u16);
        self.write_palette16(addr.wrapping_add(2), (value >> 16) as u16);
    }

    pub fn read_oam8(&self, addr: u32) -> u8 {
        self.oam[(addr as usize) % self.oam.len()]
    }

    pub fn read_oam16(&self, addr: u32) -> u16 {
        let offset = (addr as usize) % self.oam.len();
        u16::from_le_bytes([self.oam[offset], self.oam[(offset + 1) % self.oam.len()]])
    }

    pub fn read_oam32(&self, addr: u32) -> u32 {
        let offset = (addr as usize) % self.oam.len();
        self.read_oam16(offset as u32) as u32
            | ((self.read_oam16((offset + 2) as u32) as u32) << 16)
    }

    pub fn write_oam8(&mut self, _addr: u32, _value: u8) {
        // OAM has no byte-write enable; 8-bit stores are ignored.
    }

    pub fn write_oam16(&mut self, addr: u32, value: u16) {
        let len = self.oam.len();
        let offset = (addr as usize) % len;
        let bytes = value.to_le_bytes();
        self.oam[offset] = bytes[0];
        self.oam[(offset + 1) % len] = bytes[1];
    }

    pub fn write_oam32(&mut self, addr: u32, value: u32) {
        self.write_oam16(addr, value as u16);
        self.write_oam16(addr.wrapping_add(2), (value >> 16) as u16);
    }

    pub fn read_io16(&self, offset: u16) -> u16 {
        match offset {
            0x000 => self.dispcnt,
            0x004 => self.dispstat,
            0x006 => self.vcount,
            0x008..=0x00E => self.bgcnt[((offset - 0x008) / 2) as usize],
            0x010..=0x01F => {
                let index = ((offset - 0x010) / 4) as usize;
                if offset & 2 == 0 {
                    self.bghofs[index]
                } else {
                    self.bgvofs[index]
                }
            }
            0x020..=0x027 => {
                let index = ((offset - 0x020) / 2) as usize;
                match index {
                    0 => self.bgpa[0] as u16,
                    1 => self.bgpb[0] as u16,
                    2 => self.bgpc[0] as u16,
                    3 => self.bgpd[0] as u16,
                    _ => 0,
                }
            }
            0x030..=0x037 => {
                let index = ((offset - 0x030) / 2) as usize;
                match index {
                    0 => self.bgpa[1] as u16,
                    1 => self.bgpb[1] as u16,
                    2 => self.bgpc[1] as u16,
                    3 => self.bgpd[1] as u16,
                    _ => 0,
                }
            }
            0x040 => self.win0h,
            0x042 => self.win1h,
            0x044 => self.win0v,
            0x046 => self.win1v,
            0x048 => self.winin,
            0x04A => self.winout,
            0x04C => self.mosaic,
            0x050 => self.bldcnt,
            0x052 => self.bldalpha,
            0x054 => self.bldy,
            _ => 0,
        }
    }

    pub fn write_io16(&mut self, offset: u16, value: u16) {
        match offset {
            0x000 => self.dispcnt = value,
            0x004 => {
                self.dispstat = (self.dispstat & 0x0007) | (value & 0xFFF8);
            }
            0x008..=0x00E => self.bgcnt[((offset - 0x008) / 2) as usize] = value,
            0x010..=0x01F => {
                let index = ((offset - 0x010) / 4) as usize;
                if offset & 2 == 0 {
                    self.bghofs[index] = value;
                } else {
                    self.bgvofs[index] = value;
                }
            }
            0x020..=0x027 => {
                let index = ((offset - 0x020) / 2) as usize;
                match index {
                    0 => self.bgpa[0] = value as i16,
                    1 => self.bgpb[0] = value as i16,
                    2 => self.bgpc[0] = value as i16,
                    3 => self.bgpd[0] = value as i16,
                    _ => {}
                }
            }
            0x030..=0x037 => {
                let index = ((offset - 0x030) / 2) as usize;
                match index {
                    0 => self.bgpa[1] = value as i16,
                    1 => self.bgpb[1] = value as i16,
                    2 => self.bgpc[1] = value as i16,
                    3 => self.bgpd[1] = value as i16,
                    _ => {}
                }
            }
            0x040 => self.win0h = value,
            0x042 => self.win1h = value,
            0x044 => self.win0v = value,
            0x046 => self.win1v = value,
            0x048 => self.winin = value,
            0x04A => self.winout = value,
            0x04C => self.mosaic = value,
            0x050 => self.bldcnt = value,
            0x052 => self.bldalpha = value,
            0x054 => self.bldy = value,
            _ => {}
        }
    }

    pub fn write_io32(&mut self, offset: u16, value: u32) {
        match offset {
            0x028 => self.bgx[0] = sign_extend_28(value),
            0x02C => self.bgy[0] = sign_extend_28(value),
            0x038 => self.bgx[1] = sign_extend_28(value),
            0x03C => self.bgy[1] = sign_extend_28(value),
            _ => {
                self.write_io16(offset, value as u16);
                self.write_io16(offset.wrapping_add(2), (value >> 16) as u16);
            }
        }
    }

    pub fn read_io32(&self, offset: u16) -> u32 {
        match offset {
            0x028 => self.bgx[0] as u32,
            0x02C => self.bgy[0] as u32,
            0x038 => self.bgx[1] as u32,
            0x03C => self.bgy[1] as u32,
            _ => {
                self.read_io16(offset) as u32
                    | ((self.read_io16(offset.wrapping_add(2)) as u32) << 16)
            }
        }
    }

    fn vram8(&self, offset: usize) -> u8 {
        self.vram[offset % self.vram.len()]
    }

    fn vram16(&self, offset: usize) -> u16 {
        let offset = offset % self.vram.len();
        u16::from_le_bytes([self.vram[offset], self.vram[(offset + 1) % self.vram.len()]])
    }

    fn palette16(&self, index: usize) -> u16 {
        let offset = (index * 2) % self.palette.len();
        u16::from_le_bytes([
            self.palette[offset],
            self.palette[(offset + 1) % self.palette.len()],
        ])
    }

    fn oam16(&self, offset: usize) -> u16 {
        self.oam[offset % self.oam.len()] as u16
            | ((self.oam[(offset + 1) % self.oam.len()] as u16) << 8)
    }

    fn oam_i16(&self, offset: usize) -> i16 {
        self.oam16(offset) as i16
    }
}

fn insert_candidate(
    best: &mut Option<Candidate>,
    second: &mut Option<Candidate>,
    candidate: Candidate,
) {
    let better_than_best = match best {
        None => true,
        Some(current) => {
            (candidate.priority, candidate.layer.rank(), candidate.order)
                < (current.priority, current.layer.rank(), current.order)
        }
    };
    if better_than_best {
        *second = *best;
        *best = Some(candidate);
    } else {
        let better_than_second = match second {
            None => true,
            Some(current) => {
                (candidate.priority, candidate.layer.rank(), candidate.order)
                    < (current.priority, current.layer.rank(), current.order)
            }
        };
        if better_than_second {
            *second = Some(candidate);
        }
    }
}

fn mosaic_coord(value: usize, size: usize) -> usize {
    if size <= 1 {
        value
    } else {
        value - value % size
    }
}

fn mosaic_dimensions(value: u16) -> (usize, usize) {
    (
        usize::from(value & 0xF) + 1,
        usize::from((value >> 4) & 0xF) + 1,
    )
}

fn object_mosaic_dimensions(value: u16) -> (usize, usize) {
    (
        usize::from((value >> 8) & 0xF) + 1,
        usize::from((value >> 12) & 0xF) + 1,
    )
}

fn sprite_size(shape: u16, size: u16) -> Option<(usize, usize)> {
    Some(match (shape, size) {
        (0, 0) => (8, 8),
        (0, 1) => (16, 16),
        (0, 2) => (32, 32),
        (0, 3) => (64, 64),
        (1, 0) => (16, 8),
        (1, 1) => (32, 8),
        (1, 2) => (32, 16),
        (1, 3) => (64, 32),
        (2, 0) => (8, 16),
        (2, 1) => (8, 32),
        (2, 2) => (16, 32),
        (2, 3) => (32, 64),
        _ => return None,
    })
}

fn blend_alpha(src: u16, dst: u16, eva: u32, evb: u32) -> u16 {
    let r = ((((src & 0x1F) as u32) * eva + ((dst & 0x1F) as u32) * evb) / 16).min(31);
    let g =
        (((((src >> 5) & 0x1F) as u32) * eva + (((dst >> 5) & 0x1F) as u32) * evb) / 16).min(31);
    let b =
        (((((src >> 10) & 0x1F) as u32) * eva + (((dst >> 10) & 0x1F) as u32) * evb) / 16).min(31);
    (r as u16) | ((g as u16) << 5) | ((b as u16) << 10)
}

fn brightness_up(src: u16, evy: u32) -> u16 {
    let r = ((src & 0x1F) as u32 + (31 - (src & 0x1F) as u32) * evy / 16).min(31);
    let g = (((src >> 5) & 0x1F) as u32 + (31 - ((src >> 5) & 0x1F) as u32) * evy / 16).min(31);
    let b = (((src >> 10) & 0x1F) as u32 + (31 - ((src >> 10) & 0x1F) as u32) * evy / 16).min(31);
    (r as u16) | ((g as u16) << 5) | ((b as u16) << 10)
}

fn brightness_down(src: u16, evy: u32) -> u16 {
    let r = ((src & 0x1F) as u32 - (src & 0x1F) as u32 * evy / 16).min(31);
    let g = (((src >> 5) & 0x1F) as u32 - ((src >> 5) & 0x1F) as u32 * evy / 16).min(31);
    let b = (((src >> 10) & 0x1F) as u32 - ((src >> 10) & 0x1F) as u32 * evy / 16).min(31);
    (r as u16) | ((g as u16) << 5) | ((b as u16) << 10)
}

fn sign_extend_28(value: u32) -> i32 {
    let value = value & 0x0FFF_FFFF;
    if value & 0x0800_0000 != 0 {
        (value | 0xF000_0000) as i32
    } else {
        value as i32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bg_mosaic_snaps_to_the_block_top_left() {
        let mut ppu = Ppu::new();
        ppu.dispcnt = 0x0100;
        ppu.bgcnt[0] = (1 << 2) | (1 << 8) | 0x40;
        ppu.mosaic = 0x0003;
        ppu.write_palette16(0x0500_0002, 0x001F);
        ppu.write_palette16(0x0500_0004, 0x03E0);
        ppu.write_vram16(0x0800, 0);
        for (index, byte) in [0x11u8, 0x11, 0x22, 0x22].iter().enumerate() {
            ppu.write_vram8(0x4000 + index as u32, *byte);
        }

        ppu.render_line(0);
        assert_eq!(ppu.framebuffer[0], 0x001F);
        assert_eq!(ppu.framebuffer[1], 0x001F);
        assert_eq!(ppu.framebuffer[2], 0x001F);
        assert_eq!(ppu.framebuffer[3], 0x001F);
        assert_eq!(ppu.framebuffer[4], 0x03E0);
        assert_eq!(ppu.framebuffer[5], 0x03E0);
        assert_eq!(ppu.framebuffer[6], 0x03E0);
        assert_eq!(ppu.framebuffer[7], 0x03E0);
    }

    #[test]
    fn scanline_stepping_raises_hblank_and_vblank() {
        let mut ppu = Ppu::new();
        ppu.dispstat = 0x0018;
        let mut irq = Irq::default();

        let events = ppu.step(CYCLES_PER_SCANLINE, &mut irq);
        assert!(events.hblank);
        assert_ne!(irq.if_ & irq::bits::HBLANK, 0);
        assert_eq!(ppu.vcount, 1);

        for _ in 1..VISIBLE_SCANLINES {
            ppu.step(CYCLES_PER_SCANLINE, &mut irq);
        }
        assert_eq!(ppu.vcount, VISIBLE_SCANLINES);
        assert_ne!(irq.if_ & irq::bits::VBLANK, 0);
    }

    #[test]
    fn vcount_match_irq_fires() {
        let mut ppu = Ppu::new();
        ppu.dispstat = (5 << 8) | 0x0020;
        let mut irq = Irq::default();
        for _ in 0..5 {
            ppu.step(CYCLES_PER_SCANLINE, &mut irq);
        }
        assert_ne!(irq.if_ & irq::bits::VCOUNT, 0);
    }

    #[test]
    fn windows_use_separate_win0_win1_and_obj_flags() {
        let mut ppu = Ppu::new();
        ppu.dispcnt = 0x6000;
        ppu.win0h = 8 << 8;
        ppu.win0v = 160 << 8;
        ppu.win1h = (20 << 8) | 10;
        ppu.win1v = 160 << 8;
        ppu.winin = 0x0001 | (0x0002 << 8);
        ppu.winout = 0x3F;

        assert_eq!(ppu.window_mask(2, 0, false), 0x0001);
        assert_eq!(ppu.window_mask(12, 0, false), 0x0002);
        assert_eq!(ppu.window_mask(30, 0, false), 0x3F);

        ppu.dispcnt = 0x8000;
        ppu.winout = 0x3F << 8;
        assert_eq!(ppu.window_mask(0, 0, true), 0x3F);
        assert_eq!(ppu.window_mask(0, 0, false), 0x0000);
    }

    #[test]
    fn obj_window_line_marks_only_opaque_sprite_pixels() {
        let mut ppu = Ppu::new();
        ppu.dispcnt = 0x9000;
        ppu.write_oam16(0, 0x0800);
        ppu.write_oam16(2, 8);
        ppu.write_oam16(4, 0);
        for offset in 0..4 {
            ppu.write_vram8(0x1_0000 + offset, 0xFF);
        }

        let line = ppu.obj_window_line(0);
        assert!(line[8]);
        assert!(line[15]);
        assert!(!line[16]);
    }

    #[test]
    fn mode3_renders_every_pixel() {
        let mut ppu = Ppu::new();
        ppu.dispcnt = 0x0403;
        for x in 0..SCREEN_WIDTH {
            ppu.write_vram16((x * 2) as u32, x as u16);
        }
        ppu.render_line(0);
        for x in 0..SCREEN_WIDTH {
            assert_eq!(ppu.framebuffer[x], x as u16, "pixel {x}");
        }
    }
}
