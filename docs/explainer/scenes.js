// Scenes for "Cinnabar GBA, explained".
// Each key matches a segment id in script.json. The kit supplies the frame,
// the fade, the chapter counter and the type system. This file supplies the
// argument, built around one object: a single 240x160 frame.

import {
  W,
  H,
  M,
  ctx,
  C,
  F,
  IMG,
  clamp,
  lerp,
  smooth,
  easeOut,
  easeInOut,
  span,
  pulse,
  withAlpha,
  font,
  text,
  label,
  paragraph,
  revealText,
  panel,
  rrect,
  line,
  curve,
  arrow,
  dot,
  ring,
  glow,
  image,
  imageCrop,
  drawWaveforms,
  sceneLabel,
} from "/kit/render/helpers.js";

const T = {
  display: 96,
  title: 64,
  body: 32,
  caption: 24,
  label: 18,
  chrome: 14,
};

const FB = { w: 240, h: 160 };

function times(sc) {
  return sc.seg.sentences.map((s) => ({
    start: s.start - sc.start,
    end: s.end - sc.start,
    dur: s.duration,
  }));
}

function head(sc, lt, alpha = 1) {
  const dur = sc.end - sc.start;
  const p =
    smooth(span(lt, 0.35, 0.9)) *
    (1 - smooth(span(lt, dur - 0.55, dur - 0.15))) *
    alpha;
  sceneLabel(sc.seg.title ?? sc.id, M.x, M.top, p);
}

function fmt(n) {
  return Math.round(n).toLocaleString("en-US");
}

function pill(str, x, y, color = C.mint, opts = {}) {
  const { size = T.chrome, alpha = 1, align = "left", fill = true } = opts;
  font(size, 400, F.mono);
  ctx.letterSpacing = "2px";
  const labelStr = str.toUpperCase();
  const w = ctx.measureText(labelStr).width + 26;
  const px = align === "center" ? x - w / 2 : align === "right" ? x - w : x;
  const h = size + 16;
  ctx.save();
  ctx.globalAlpha *= alpha;
  if (fill) {
    rrect(px, y - size - 7, w, h, 999);
    ctx.fillStyle = withAlpha(color, 0.1);
    ctx.fill();
    ctx.strokeStyle = withAlpha(color, 0.35);
    ctx.lineWidth = 1.2;
    ctx.stroke();
  }
  ctx.restore();
  text(labelStr, px + 13, y, { size, family: F.mono, color, ls: 2, alpha });
  return w;
}

function check(x, y, p, color = C.mint) {
  if (p <= 0) return;
  ctx.save();
  ctx.globalAlpha *= p;
  ctx.strokeStyle = color;
  ctx.lineWidth = 2.4;
  ctx.lineCap = "round";
  ctx.lineJoin = "round";
  ctx.beginPath();
  ctx.moveTo(x - 8, y);
  ctx.lineTo(x - 2, y + 7);
  ctx.lineTo(x + 10, y - 8);
  ctx.stroke();
  ctx.restore();
}

function cross(x, y, p, color = C.coral, size = 8) {
  if (p <= 0) return;
  ctx.save();
  ctx.globalAlpha *= p;
  ctx.strokeStyle = color;
  ctx.lineWidth = 2.2;
  ctx.lineCap = "round";
  ctx.beginPath();
  ctx.moveTo(x - size, y - size);
  ctx.lineTo(x + size, y + size);
  ctx.moveTo(x + size, y - size);
  ctx.lineTo(x - size, y + size);
  ctx.stroke();
  ctx.restore();
}

function codeCard(lines, x, y, w, opts = {}) {
  const {
    size = T.label,
    lh = 1.75,
    title = null,
    accent = C.mint,
    alpha = 1,
    fill = "rgba(6,9,18,0.86)",
    stroke = "rgba(244,241,234,0.11)",
    r = 14,
    pad = 22,
  } = opts;
  const lineH = size * lh;
  const padTop = title ? 58 : 30;
  const h = lines.length * lineH + padTop + 12;
  panel(x, y, w, h, { r, fill, stroke, alpha });
  if (title) label(title, x + pad, y + 34, { size: T.chrome, alpha });
  lines.forEach((ln, i) => {
    const entry = typeof ln === "string" ? { t: ln } : ln;
    const color = entry.color ?? (entry.t.trim().startsWith("//") ? C.faint : accent);
    text(entry.t, x + pad, y + padTop + i * lineH, {
      size,
      family: F.mono,
      color,
      alpha: alpha * (entry.alpha ?? 1),
    });
  });
  return h;
}

// Rounded, nearest-neighbour framebuffer image. Source images are 240x160.
function pixelImage(name, x, y, w, h, alpha = 1, r = 12, zoom = 1) {
  const img = IMG[name];
  if (!img) return;
  ctx.save();
  ctx.globalAlpha *= alpha;
  rrect(x, y, w, h, r);
  ctx.clip();
  ctx.imageSmoothingEnabled = false;
  const dw = w * zoom;
  const dh = h * zoom;
  ctx.drawImage(img, x + (w - dw) / 2, y + (h - dh) / 2, dw, dh);
  ctx.restore();
}

// Draw only the top `lines` of a 240x160 frame, scaled to w x h.
function scanImage(name, x, y, w, h, lines, alpha = 1, r = 12) {
  const img = IMG[name];
  if (!img || lines <= 0) return;
  const n = clamp(lines, 0, FB.h);
  const scaleY = h / FB.h;
  ctx.save();
  ctx.globalAlpha *= alpha;
  rrect(x, y, w, h, r);
  ctx.clip();
  ctx.imageSmoothingEnabled = false;
  ctx.drawImage(img, 0, 0, FB.w, n, x, y, w, n * scaleY);
  ctx.restore();
}

function bracket(x1, x2, y, str, color, alpha = 1, opts = {}) {
  if (alpha <= 0) return;
  const { below = false, size = T.chrome } = opts;
  const d = below ? 1 : -1;
  ctx.save();
  ctx.globalAlpha *= alpha;
  line(x1, y, x2, y, color, 1.2);
  line(x1, y, x1, y + d * 7, color, 1.2);
  line(x2, y, x2, y + d * 7, color, 1.2);
  ctx.restore();
  text(str, (x1 + x2) / 2, y + d * (size + 10), {
    size,
    family: F.mono,
    color,
    align: "center",
    alpha,
    ls: 1.5,
  });
}

function drawSquare(x, y, w, h, phase, duty, color, lw = 2.2, alpha = 1) {
  ctx.save();
  ctx.globalAlpha *= alpha;
  ctx.strokeStyle = color;
  ctx.lineWidth = lw;
  ctx.lineJoin = "miter";
  ctx.beginPath();
  const cycles = 4;
  const n = 320;
  let prevOn = null;
  let prevY = y + h / 2;
  for (let i = 0; i <= n; i++) {
    const p = (((i / n) * cycles + phase * cycles) % cycles) / cycles;
    const on = p < duty;
    const px = x + (i / n) * w;
    const py = y + h / 2 - (on ? 1 : -1) * h * 0.32;
    if (i === 0) {
      ctx.moveTo(px, py);
    } else if (on !== prevOn) {
      ctx.lineTo(px, prevY);
      ctx.lineTo(px, py);
    } else {
      ctx.lineTo(px, py);
    }
    prevOn = on;
    prevY = py;
  }
  ctx.stroke();
  ctx.restore();
}

function drawStepWave(x, y, w, h, phase, color, lw = 2.2, alpha = 1) {
  ctx.save();
  ctx.globalAlpha *= alpha;
  ctx.strokeStyle = color;
  ctx.lineWidth = lw;
  ctx.lineJoin = "round";
  ctx.beginPath();
  const n = 32;
  const step = w / n;
  for (let i = 0; i < n; i++) {
    const v = 0.5 + 0.42 * Math.sin((i / n) * Math.PI * 4 + phase * Math.PI * 2);
    const py = y + h - v * h;
    const px = x + i * step;
    if (i === 0) ctx.moveTo(px, py);
    else {
      ctx.lineTo(px, py);
    }
    ctx.lineTo(px + step, py);
  }
  ctx.stroke();
  ctx.restore();
}

function drawNoise(x, y, w, h, phase, color, lw = 1.6, alpha = 1) {
  ctx.save();
  ctx.globalAlpha *= alpha;
  ctx.strokeStyle = color;
  ctx.lineWidth = lw;
  ctx.lineJoin = "miter";
  ctx.beginPath();
  const n = 64;
  const step = w / n;
  const shift = Math.floor(phase * 32);
  for (let i = 0; i < n; i++) {
    const s = Math.sin((i + shift) * 127.1 + 311.7) * 43758.5453;
    const v = s - Math.floor(s);
    const py = y + h / 2 - (v * 2 - 1) * h * 0.34;
    const px = x + i * step;
    if (i === 0) ctx.moveTo(px, py);
    else ctx.lineTo(px, py);
  }
  ctx.stroke();
  ctx.restore();
}

function meter(x, y, w, h, level, color, alpha = 1) {
  ctx.save();
  ctx.globalAlpha *= alpha;
  rrect(x, y, w, h, h / 2);
  ctx.fillStyle = "rgba(244,241,234,0.08)";
  ctx.fill();
  const lw = Math.max(3, w * clamp(level, 0, 1));
  rrect(x, y, lw, h, h / 2);
  ctx.fillStyle = color;
  ctx.fill();
  ctx.restore();
}

function iconChip(x, y, color, alpha = 1) {
  ctx.save();
  ctx.globalAlpha *= alpha;
  ctx.strokeStyle = color;
  ctx.lineWidth = 1.3;
  rrect(x - 13, y - 13, 26, 26, 5);
  ctx.stroke();
  for (let i = 0; i < 4; i++) {
    line(x - 18, y - 8 + i * 5.3, x - 13, y - 8 + i * 5.3, color, 1);
    line(x + 13, y - 8 + i * 5.3, x + 18, y - 8 + i * 5.3, color, 1);
  }
  ctx.restore();
}

function iconScreen(x, y, color, alpha = 1) {
  ctx.save();
  ctx.globalAlpha *= alpha;
  ctx.strokeStyle = color;
  ctx.lineWidth = 1.3;
  rrect(x - 18, y - 12, 36, 24, 4);
  ctx.stroke();
  line(x - 13, y + 4, x + 13, y + 4, withAlpha(color, 0.5), 1);
  ctx.restore();
}

function iconSound(x, y, color, alpha = 1) {
  ctx.save();
  ctx.globalAlpha *= alpha;
  ctx.strokeStyle = color;
  ctx.lineWidth = 1.4;
  ctx.beginPath();
  ctx.moveTo(x - 16, y - 6);
  ctx.lineTo(x - 9, y - 6);
  ctx.lineTo(x - 2, y - 13);
  ctx.lineTo(x - 2, y + 13);
  ctx.lineTo(x - 9, y + 6);
  ctx.lineTo(x - 16, y + 6);
  ctx.closePath();
  ctx.stroke();
  ctx.beginPath();
  ctx.arc(x - 2, y, 10, -0.8, 0.8);
  ctx.stroke();
  ctx.restore();
}

function iconCart(x, y, color, alpha = 1) {
  ctx.save();
  ctx.globalAlpha *= alpha;
  ctx.strokeStyle = color;
  ctx.lineWidth = 1.3;
  rrect(x - 14, y - 16, 28, 30, 4);
  ctx.stroke();
  line(x - 8, y - 10, x + 8, y - 10, color, 1);
  line(x - 8, y - 4, x + 8, y - 4, color, 1);
  line(x - 8, y + 2, x + 2, y + 2, color, 1);
  ctx.restore();
}

function iconTimer(x, y, color, alpha = 1) {
  ctx.save();
  ctx.globalAlpha *= alpha;
  ctx.strokeStyle = color;
  ctx.lineWidth = 1.4;
  ctx.beginPath();
  ctx.arc(x, y + 2, 12, 0, Math.PI * 2);
  ctx.stroke();
  line(x, y + 2, x, y - 6, color, 1.4);
  line(x, y + 2, x + 6, y + 4, color, 1.4);
  line(x - 5, y - 14, x + 5, y - 14, color, 1.4);
  ctx.restore();
}

function iconDma(x, y, color, alpha = 1) {
  ctx.save();
  ctx.globalAlpha *= alpha;
  ctx.strokeStyle = color;
  ctx.lineWidth = 1.4;
  line(x - 16, y - 6, x + 14, y - 6, color, 1.4);
  arrow(x + 6, y - 6, x + 16, y - 6, color, 1.4, 7);
  line(x + 16, y + 7, x - 14, y + 7, color, 1.4);
  arrow(x - 6, y + 7, x - 16, y + 7, color, 1.4, 7);
  ctx.restore();
}

// ---------------------------------------------------------------- frame

function frameScene(lt, sc) {
  const s = times(sc);
  const t0 = s[0].start;
  const t1 = s[1].start;
  const t2 = s[2].start;
  const t3 = s[3].start;

  head(sc, lt);

  const enter = easeOut(span(lt, t0, t0 + 1.3));
  const card = easeInOut(span(lt, t3 - 0.25, t3 + 0.75));
  const shell = easeOut(span(lt, t2 - 0.1, t2 + 1.1)) * (1 - smooth(span(lt, t3 - 0.3, t3 + 0.3)));
  const dims = smooth(span(lt, t1 - 0.2, t1 + 0.6)) * (1 - smooth(span(lt, t2 - 0.3, t2 + 0.3)));

  const w = lerp(960, 360, card);
  const h = w * 0.6667;
  const x = lerp(480, 110, card);
  const y = lerp(202, 212, card);

  // GBA shell: the frame becomes the screen of a drawn console.
  if (shell > 0) {
    const bx = x - 190;
    const by = y - 70;
    const bw = w + 380;
    const bh = h + 140;
    panel(bx, by, bw, bh, {
      r: 58,
      fill: "rgba(12,16,32,0.55)",
      stroke: withAlpha(C.coral, 0.22),
      lw: 1.6,
      alpha: shell,
      glow: withAlpha(C.coral, 0.07),
    });
    // d-pad
    const cx = bx + 95;
    const cy = by + bh * 0.56;
    rrect(cx - 42, cy - 14, 84, 28, 6);
    ctx.fillStyle = withAlpha(C.ink, 0.14 * shell);
    ctx.fill();
    rrect(cx - 14, cy - 42, 28, 84, 6);
    ctx.fill();
    dot(cx, cy, 5, withAlpha(C.ink, 0.25), shell);
    label("D-PAD", cx, cy + 62, { size: T.chrome, color: C.faint, align: "center", alpha: shell });
    // face buttons
    const ax = bx + bw - 140;
    const ay = by + bh * 0.48;
    dot(ax, ay, 24, withAlpha(C.ink, 0.1), shell);
    ring(ax, ay, 24, withAlpha(C.ink, 0.3), 1.2, shell);
    text("A", ax, ay + 8, { size: T.label, family: F.mono, color: C.ink, align: "center", alpha: shell * 0.8 });
    dot(ax + 62, ay + 34, 24, withAlpha(C.ink, 0.1), shell);
    ring(ax + 62, ay + 34, 24, withAlpha(C.ink, 0.3), 1.2, shell);
    text("B", ax + 62, ay + 42, { size: T.label, family: F.mono, color: C.ink, align: "center", alpha: shell * 0.8 });
    // start / select
    rrect(bx + bw / 2 - 46, by + bh - 60, 34, 8, 4);
    ctx.fillStyle = withAlpha(C.ink, 0.25 * shell);
    ctx.fill();
    rrect(bx + bw / 2 + 12, by + bh - 60, 34, 8, 4);
    ctx.fill();
    // speaker slots
    for (let i = 0; i < 6; i++) {
      line(ax + 22 + i * 9, ay + 92, ax + 32 + i * 9, ay + 104, withAlpha(C.ink, 0.2), 1.4, null, shell);
    }
  }

  // the frame itself
  if (enter > 0) {
    ctx.save();
    ctx.globalAlpha *= enter;
    glow(x + w / 2, y + h / 2, w * 0.58, C.coral, 0.13 * (1 - card * 0.5));
    ctx.restore();
    panel(x - 7, y - 7, w + 14, h + 14, {
      r: 16,
      fill: "rgba(4,6,14,0.85)",
      stroke: withAlpha(C.ink, 0.16),
      lw: 1.3,
      alpha: enter,
    });
    pixelImage("demo", x, y, w, h, enter, 9, 1 + (1 - enter) * 0.04);
    if (card > 0.4) {
      rrect(x - 7, y - 7, w + 14, h + 14, 16);
      ctx.strokeStyle = withAlpha(C.coral, 0.6 * card);
      ctx.lineWidth = 1.6;
      ctx.stroke();
    }
  }

  // metadata above the frame, before the title card
  const meta = (1 - smooth(span(lt, t2 - 0.4, t2 + 0.2))) * enter;
  text("CINNABAR LOGO / GBA", x, y - 22, {
    size: T.chrome,
    family: F.mono,
    color: C.dim,
    ls: 2,
    alpha: meta,
  });
  text("HEADLESS CAPTURE 6 / 240x160", x + w, y - 22, {
    size: T.chrome,
    family: F.mono,
    color: C.faint,
    align: "right",
    alpha: meta,
  });

  // dimensions
  if (dims > 0) {
    const by = y + h + 30;
    line(x, by, x + w, by, withAlpha(C.mint, 0.5), 1.2, null, dims);
    line(x, by - 8, x, by + 8, withAlpha(C.mint, 0.5), 1.2, null, dims);
    line(x + w, by - 8, x + w, by + 8, withAlpha(C.mint, 0.5), 1.2, null, dims);
    text("240 px", x + w / 2, by + 28, { size: T.label, family: F.mono, color: C.mint, align: "center", alpha: dims });
    const lx = x - 30;
    line(lx, y, lx, y + h, withAlpha(C.mint, 0.5), 1.2, null, dims);
    line(lx - 8, y, lx + 8, y, withAlpha(C.mint, 0.5), 1.2, null, dims);
    line(lx - 8, y + h, lx + 8, y + h, withAlpha(C.mint, 0.5), 1.2, null, dims);
    text("160", lx - 12, y + h / 2 - 2, { size: T.label, family: F.mono, color: C.mint, align: "right", alpha: dims });
    text("px", lx - 12, y + h / 2 + 18, { size: T.chrome, family: F.mono, color: withAlpha(C.mint, 0.6), align: "right", alpha: dims });
    const hz = smooth(span(lt, t1 + 1.8, t1 + 2.6));
    if (hz > 0) {
      pill("59.73 Hz", x + w - 8, y + 14, C.coral, { align: "right", alpha: hz });
    }
  }

  // title card
  if (card > 0) {
    const p = easeOut(span(lt, t3 + 0.05, t3 + 0.9));
    const tx = 600;
    rrect(tx, 258, 84 * p, 5, 3);
    ctx.fillStyle = C.coral;
    ctx.fill();
    text("CINNABAR", tx, 360, { size: T.display, weight: 600, color: C.ink, alpha: p });
    text("GBA", tx, 462, { size: T.display, weight: 700, color: C.coral, alpha: p });
    text("a clean-room Game Boy Advance emulator", tx, 518, {
      size: T.caption,
      color: C.dim,
      alpha: p,
    });
    pill("RUST", tx, 574, C.mint, { alpha: p });
    pill("NO BIOS", tx + 92, 574, C.coral, { alpha: p });
    pill("MIT", tx + 226, 574, C.peach, { alpha: p });
    text("240 x 160  /  59.7275 Hz  /  280,896 cycles", tx, 650, {
      size: T.chrome,
      family: F.mono,
      color: C.faint,
      ls: 1.2,
      alpha: p * 0.9,
    });
    label("THE OBJECT", 110, 480, { size: T.chrome, color: C.coral, alpha: p });
    text("one frame", 110, 520, { size: T.body, family: F.serif, style: "italic", color: C.dim, alpha: p });
  }
}

// ---------------------------------------------------------------- machine

function machineScene(lt, sc) {
  const s = times(sc);
  const t0 = s[0].start;
  const t1 = s[1].start;
  const t2 = s[2].start;

  head(sc, lt);

  const busP = easeOut(span(lt, t0 + 0.2, t0 + 1.1));
  const busX = 150;
  const busY = 594;
  const busW = 1120;

  if (busP > 0) {
    ctx.save();
    ctx.globalAlpha *= busP;
    rrect(busX, busY - 9, busW, 18, 9);
    ctx.fillStyle = withAlpha(C.coral, 0.16);
    ctx.fill();
    ctx.strokeStyle = withAlpha(C.coral, 0.7);
    ctx.lineWidth = 1.4;
    ctx.stroke();
    const g = ctx.createLinearGradient(busX, 0, busX + busW, 0);
    g.addColorStop(0, withAlpha(C.coral, 0.1));
    g.addColorStop(0.5, withAlpha(C.coral, 0.85));
    g.addColorStop(1, withAlpha(C.coral, 0.1));
    ctx.fillStyle = g;
    ctx.beginPath();
    ctx.roundRect(busX + 1, busY - 2, busW * easeOut(span(lt, t0 + 0.2, t0 + 1.6)), 4, 2);
    ctx.fill();
    ctx.restore();
    label("ONE BUS", busX + 18, busY - 22, { color: C.coral, alpha: busP });
    text("every chip shares it", busX + busW, busY - 22, {
      size: T.chrome,
      family: F.mono,
      color: C.faint,
      align: "right",
      alpha: busP,
    });
  }

  const chips = [
    { x: 150, y: 250, name: "CPU", sub: "ARM7TDMI", detail: "16.78 MHz", icon: iconChip, color: C.coral },
    { x: 500, y: 250, name: "PPU", sub: "240 x 160", detail: "modes 0-5", icon: iconScreen, color: C.peach },
    { x: 850, y: 250, name: "APU", sub: "4 PSG + 2 DMA", detail: "48 kHz stereo", icon: iconSound, color: C.mint },
    { x: 150, y: 742, name: "CART", sub: "ROM + save", detail: "header, SRAM, Flash, EEPROM", icon: iconCart, color: C.cyan },
    { x: 500, y: 742, name: "DMA", sub: "4 channels", detail: "memory without the CPU", icon: iconDma, color: C.violet },
    { x: 850, y: 742, name: "TIMERS", sub: "4, cascade", detail: "and the IRQ controller", icon: iconTimer, color: C.peach },
  ];

  const cpuGlow = smooth(span(lt, t2 - 0.3, t2 + 0.6));
  chips.forEach((c, i) => {
    const p = easeOut(span(lt, t1 + 0.05 + i * 0.16, t1 + 0.65 + i * 0.16));
    if (p <= 0) return;
    const active = i === 0;
    const dim = active ? 1 : 1 - 0.55 * cpuGlow;
    const a = p * (active ? 1 : dim);
    // connection to the bus
    const top = i < 3;
    const linkX = top ? c.x + 150 : c.x + (i === 3 ? 80 : 100);
    line(linkX, top ? c.y + 118 : c.y + 2, linkX, top ? busY - 10 : busY + 10, withAlpha(c.color, 0.35), 1.2, null, a);
    dot(linkX, top ? busY - 10 : busY + 10, 3, c.color, a);
    panel(c.x, c.y, 300, 120, {
      r: 14,
      fill: "rgba(9,13,26,0.72)",
      stroke: active && cpuGlow > 0.2 ? withAlpha(C.coral, 0.65) : withAlpha(c.color, 0.22),
      lw: active && cpuGlow > 0.2 ? 1.8 : 1.2,
      alpha: a,
      glow: active && cpuGlow > 0.2 ? withAlpha(C.coral, 0.12) : null,
    });
    c.icon(c.x + 30, c.y + 32, c.color, a);
    text(c.name, c.x + 56, c.y + 42, { size: T.caption, family: F.mono, color: c.color, ls: 2, alpha: a });
    text(c.sub, c.x + 22, c.y + 78, { size: T.label, family: F.mono, color: C.ink, alpha: a });
    text(c.detail, c.x + 22, c.y + 102, { size: T.chrome, family: F.mono, color: C.faint, alpha: a });
  });

  // frame budget panel
  const tp = smooth(span(lt, t2 + 0.5, t2 + 1.2));
  if (tp > 0) {
    const px = 1320;
    const py = 226;
    panel(px, py, 490, 540, {
      r: 16,
      fill: "rgba(10,14,28,0.68)",
      stroke: withAlpha(C.coral, 0.26),
      alpha: tp,
    });
    label("FRAME BUDGET", px + 26, py + 38, { color: C.coral, alpha: tp });
    const countP = easeOut(span(lt, t2 + 0.8, t2 + 4.2));
    const n = 280896 * countP;
    text(fmt(n), px + 26, py + 148, {
      size: T.title,
      weight: 500,
      family: F.mono,
      color: C.ink,
      alpha: tp,
    });
    text("CYCLES PER FRAME", px + 26, py + 184, { size: T.chrome, family: F.mono, color: C.dim, ls: 2, alpha: tp });
    line(px + 26, py + 220, px + 464, py + 220, "rgba(244,241,234,0.12)", 1, null, tp);
    text("16,777,216 Hz ÷ 59.7275", px + 26, py + 264, { size: T.label, family: F.mono, color: C.dim, alpha: tp });
    text("= 280,896", px + 26, py + 304, { size: T.caption, family: F.mono, color: C.mint, alpha: tp });
    text("228 scanlines x 1,232 cycles", px + 26, py + 350, { size: T.chrome, family: F.mono, color: C.faint, alpha: tp });
    text("at 16.78 MHz", px + 26, py + 378, { size: T.chrome, family: F.mono, color: C.faint, alpha: tp });
    // a slim budget bar
    const bw = 438;
    rrect(px + 26, py + 420, bw, 10, 5);
    ctx.fillStyle = "rgba(244,241,234,0.08)";
    ctx.fill();
    rrect(px + 26, py + 420, bw * countP, 10, 5);
    ctx.fillStyle = C.coral;
    ctx.fill();
    text("one frame", px + 26, py + 460, { size: T.chrome, family: F.mono, color: C.dim, alpha: tp });
    text("59.7275 Hz", px + 464, py + 460, { size: T.chrome, family: F.mono, color: C.coral, align: "right", alpha: tp });
  }
}

// ---------------------------------------------------------------- bus

function busScene(lt, sc) {
  const s = times(sc);
  const t0 = s[0].start;
  const t1 = s[1].start;
  const t2 = s[2].start;
  const t3 = s[3].start;

  head(sc, lt);

  const topP = easeOut(span(lt, t0 + 0.2, t0 + 1.0));
  const px = 110;
  const py = 178;

  if (topP > 0) {
    panel(px, py, 220, 110, { r: 12, stroke: withAlpha(C.coral, 0.25), alpha: topP });
    text("CPU", px + 22, py + 44, { size: T.caption, family: F.mono, color: C.coral, alpha: topP });
    text("ARM7TDMI", px + 22, py + 78, { size: T.chrome, family: F.mono, color: C.dim, alpha: topP });

    arrow(px + 228, py + 55, px + 300, py + 55, withAlpha(C.ink, 0.4), 1.4, 8, topP);

    panel(px + 310, py, 300, 110, {
      r: 12,
      fill: "rgba(20,12,18,0.75)",
      stroke: withAlpha(C.coral, 0.6),
      alpha: topP,
      glow: withAlpha(C.coral, 0.08),
    });
    text("BUS", px + 332, py + 46, { size: T.caption, family: F.mono, color: C.coral, ls: 2, alpha: topP });
    text("reads the top byte", px + 332, py + 80, { size: T.chrome, family: F.mono, color: C.dim, alpha: topP });

    arrow(px + 620, py + 55, px + 700, py + 55, withAlpha(C.ink, 0.4), 1.4, 8, topP);

    panel(px + 710, py, 300, 110, { r: 12, stroke: withAlpha(C.mint, 0.25), alpha: topP });
    text("DEVICE", px + 732, py + 44, { size: T.caption, family: F.mono, color: C.mint, alpha: topP });
    text("RAM, PPU, cartridge...", px + 732, py + 78, { size: T.chrome, family: F.mono, color: C.dim, alpha: topP });
  }

  // address card
  const addrP = easeOut(span(lt, t1 - 0.2, t1 + 0.6));
  const ax = 110;
  const ay = 340;
  if (addrP > 0) {
    panel(ax, ay, 880, 150, {
      r: 14,
      fill: "rgba(6,9,18,0.86)",
      stroke: "rgba(244,241,234,0.1)",
      alpha: addrP,
    });
    label("ADDRESS", ax + 24, ay + 34, { color: C.dim, alpha: addrP });
    text("the top byte selects the chip", ax + 856, ay + 34, {
      size: T.chrome,
      family: F.mono,
      color: C.faint,
      align: "right",
      alpha: addrP,
    });
  }

  const rows = [
    { byte: "02", name: "EWRAM", range: "0x02000000", size: "256 KB", color: C.coral, key: "ram" },
    { byte: "03", name: "IWRAM", range: "0x03000000", size: "32 KB", color: C.coral, key: "ram" },
    { byte: "04", name: "I/O", range: "0x04000000", size: "registers", color: C.mint, key: "io" },
    { byte: "05", name: "PALETTE", range: "0x05000000", size: "1 KB", color: C.mint, key: "io" },
    { byte: "06", name: "VRAM", range: "0x06000000", size: "96 KB", color: C.mint, key: "io" },
    { byte: "07", name: "OAM", range: "0x07000000", size: "1 KB", color: C.mint, key: "io" },
    { byte: "08-0D", name: "ROM", range: "0x08000000", size: "up to 32 MB", color: C.peach, key: "rom" },
    { byte: "0E-0F", name: "SAVE", range: "0x0E000000", size: "up to 128 KB", color: C.violet, key: "rom" },
  ];

  // which region the narration is naming
  let focus = null;
  if (lt >= t2 - 0.2) {
    const a = lt - t2;
    if (a < 2.3) focus = "ram";
    else if (a < 4.5) focus = "io";
    else focus = "rom";
  }

  const mapP = easeOut(span(lt, t1 + 0.2, t1 + 1.1));
  const mapX = 1160;
  const mapY = 168;
  const mapW = 650;
  const rowH = 78;
  const gap = 9;
  rows.forEach((r, i) => {
    const p = easeOut(span(lt, t1 + 0.3 + i * 0.09, t1 + 0.8 + i * 0.09)) * mapP;
    if (p <= 0) return;
    const y = mapY + i * (rowH + gap);
    const on = focus === r.key;
    const dim = focus && !on ? 0.35 : 1;
    panel(mapX, y, mapW, rowH, {
      r: 10,
      fill: on ? withAlpha(r.color, 0.13) : "rgba(9,13,26,0.62)",
      stroke: on ? withAlpha(r.color, 0.7) : "rgba(244,241,234,0.09)",
      lw: on ? 1.7 : 1.1,
      alpha: p * dim,
      glow: on ? withAlpha(r.color, 0.1) : null,
    });
    text(r.byte, mapX + 22, y + 33, { size: T.caption, family: F.mono, color: r.color, alpha: p * dim });
    text(r.range, mapX + 22, y + 60, { size: T.chrome, family: F.mono, color: C.faint, alpha: p * dim });
    text(r.name, mapX + 220, y + 40, { size: T.caption, family: F.mono, color: C.ink, ls: 2, alpha: p * dim });
    text(r.size, mapX + mapW - 22, y + 40, { size: T.chrome, family: F.mono, color: C.dim, align: "right", alpha: p * dim });
    if (on) dot(mapX - 16, y + 39, 5, r.color, p, 12);
  });

  // live address
  const addrState = focus === "ram" ? { hi: "02", full: "0x0200_0000" } : focus === "io" ? { hi: "04", full: "0x0400_0000" } : focus === "rom" ? { hi: "08", full: "0x0800_0000" } : { hi: "04", full: "0x0400_0000" };
  text("0x", ax + 24, ay + 106, { size: T.title, family: F.mono, color: C.faint, alpha: addrP });
  text(addrState.hi, ax + 106, ay + 106, {
    size: T.title,
    family: F.mono,
    color: focus ? C.coral : C.ink,
    alpha: addrP,
  });
  text(addrState.full.slice(4), ax + 188, ay + 106, { size: T.title, family: F.mono, color: C.ink, alpha: addrP });
  text("address", ax + 24, ay + 136, { size: T.chrome, family: F.mono, color: C.faint, alpha: addrP });

  // the whole switchboard
  const allP = smooth(span(lt, t3 - 0.2, t3 + 0.5));
  if (allP > 0) {
    for (let i = 0; i < 6; i++) {
      const yy = 594 + Math.sin(lt * 0.8 + i) * 2;
      dot(1040 - 70 - i * 22, yy, 3, withAlpha(C.coral, 0.5), allP * 0.7);
    }
    text("one switchboard, every chip behind it", 110, 570, {
      size: T.body,
      color: C.ink,
      alpha: allP,
    });
  }
}

// ---------------------------------------------------------------- cpu

function cpuScene(lt, sc) {
  const s = times(sc);
  const t0 = s[0].start;
  const t1 = s[1].start;
  const t2 = s[2].start;
  const t3 = s[3].start;

  head(sc, lt);

  const armP = easeOut(span(lt, t0 + 0.2, t0 + 1.0));
  const thumbP = easeOut(span(lt, t1 + 0.1, t1 + 1.0));
  const fieldsP = smooth(span(lt, t2 - 0.2, t2 + 0.7));
  const pipeP = smooth(span(lt, t3 + 0.25, t3 + 0.9));
  const wordsOut = 1 - smooth(span(lt, t3 - 0.1, t3 + 0.2));
  const cardFade = 1 - 0.68 * pipeP;

  // language cards
  if (armP > 0) {
    panel(110, 170, 520, 150, {
      r: 14,
      stroke: withAlpha(C.coral, 0.32),
      alpha: armP * cardFade,
    });
    text("ARM", 138, 236, { size: T.title, weight: 600, color: C.coral, alpha: armP * cardFade });
    text("32-bit instructions", 138, 284, { size: T.caption, color: C.dim, alpha: armP * cardFade });
  }
  if (thumbP > 0) {
    panel(670, 170, 520, 150, {
      r: 14,
      stroke: withAlpha(C.mint, 0.32),
      alpha: thumbP * cardFade,
    });
    text("THUMB", 698, 236, { size: T.title, weight: 600, color: C.mint, alpha: thumbP * cardFade });
    text("16-bit instructions", 698, 284, { size: T.caption, color: C.dim, alpha: thumbP * cardFade });
  }

  // bit strips
  const cell = 32;
  const armX = 110;
  const armY = 420;
  if (armP > 0 && wordsOut > 0) {
    text("ADD r0, r1, r2", armX, 380, { size: T.caption, family: F.mono, color: C.ink, alpha: armP * wordsOut });
    text("0xE0810002", armX + 32 * cell, 380, { size: T.caption, family: F.mono, color: C.faint, align: "right", alpha: armP * wordsOut });
    for (let i = 0; i < 32; i++) {
      let col = "rgba(244,241,234,0.16)";
      if (fieldsP > 0) {
        if (i < 4) col = withAlpha(C.coral, 0.8);
        else if (i >= 7 && i < 11) col = withAlpha(C.mint, 0.8);
        else if (i === 11) col = withAlpha(C.peach, 0.8);
        else if (i >= 12 && i < 16) col = withAlpha(C.cyan, 0.8);
        else if (i >= 16 && i < 20) col = withAlpha(C.violet, 0.8);
        else if (i >= 20) col = withAlpha(C.peach, 0.42);
      }
      const p = easeOut(span(lt, t1 + i * 0.015, t1 + 0.3 + i * 0.015));
      rrect(armX + i * cell + 1, armY, cell - 2, 46, 2);
      ctx.fillStyle = col;
      ctx.globalAlpha = armP * wordsOut * p;
      ctx.fill();
      ctx.globalAlpha = 1;
    }
    if (fieldsP > 0) {
      bracket(armX, armX + 4 * cell, armY + 50, "COND", C.coral, fieldsP, { below: true });
      bracket(armX + 7 * cell, armX + 11 * cell, armY + 50, "OPCODE", C.mint, fieldsP, { below: true });
      bracket(armX + 12 * cell, armX + 16 * cell, armY + 50, "RN", C.cyan, fieldsP, { below: true });
      bracket(armX + 16 * cell, armX + 20 * cell, armY + 50, "RD", C.violet, fieldsP, { below: true });
      bracket(armX + 20 * cell, armX + 32 * cell, armY + 50, "OPERAND2", C.peach, fieldsP, { below: true });
    }
  }

  // thumb strip, half the width
  if (thumbP > 0 && wordsOut > 0) {
    const ty = 566;
    text("a 16-bit Thumb word", 110, 552, { size: T.chrome, family: F.mono, color: C.mint, alpha: thumbP * wordsOut });
    for (let i = 0; i < 16; i++) {
      const p = easeOut(span(lt, t1 + 0.4 + i * 0.02, t1 + 0.7 + i * 0.02));
      rrect(110 + i * cell + 1, ty, cell - 2, 46, 2);
      ctx.fillStyle = withAlpha(C.mint, 0.35);
      ctx.globalAlpha = thumbP * wordsOut * p;
      ctx.fill();
      ctx.globalAlpha = 1;
    }
    text("half the space", 110 + 16 * cell + 26, ty + 30, {
      size: T.label,
      family: F.mono,
      color: C.mint,
      alpha: thumbP * wordsOut,
    });
  }

  // pipeline diagram
  if (pipeP > 0) {
    const armY = 380;
    const boxes = [
      { x: 110, tag: "EXECUTE", addr: "0x08000000", body: "add r0, r1, r2", color: C.coral },
      { x: 410, tag: "DECODE", addr: "0x08000004", body: "decode", color: C.peach },
      { x: 710, tag: "FETCH", addr: "0x08000008", body: "next instruction", color: C.mint },
    ];
    text("ARM / 32-BIT", 110, 356, { size: T.chrome, family: F.mono, color: C.coral, ls: 3, alpha: pipeP });
    boxes.forEach((b, i) => {
      const p = easeOut(span(lt, t3 + 0.3 + i * 0.16, t3 + 0.85 + i * 0.16));
      panel(b.x, armY, 250, 110, {
        r: 10,
        fill: "rgba(10,14,28,0.7)",
        stroke: withAlpha(b.color, i === 2 ? 0.8 : 0.25),
        alpha: pipeP * p,
        glow: i === 2 ? withAlpha(b.color, 0.1) : null,
      });
      text(b.tag, b.x + 20, armY + 40, { size: T.chrome, family: F.mono, color: b.color, ls: 2, alpha: pipeP * p });
      text(b.body, b.x + 20, armY + 78, {
        size: T.label,
        family: F.mono,
        color: i === 0 ? C.ink : C.dim,
        alpha: pipeP * p,
      });
      text(b.addr, b.x + 20, armY + 136, { size: T.chrome, family: F.mono, color: C.faint, alpha: pipeP * p });
      if (i < 2) arrow(b.x + 258, armY + 55, b.x + 292, armY + 55, withAlpha(C.ink, 0.3), 1.2, 7, pipeP * p);
    });
    // PC points two instructions ahead, at +8
    const pcX = 835;
    arrow(pcX, 344, pcX, 377, C.coral, 1.6, 8, pipeP);
    text("PC", pcX + 14, 350, { size: T.label, family: F.mono, color: C.coral, alpha: pipeP });
    text("= 0x08000008", pcX + 14, 372, { size: T.chrome, family: F.mono, color: C.dim, alpha: pipeP });

    // thumb row
    text("THUMB / 16-BIT", 110, 588, { size: T.chrome, family: F.mono, color: C.mint, ls: 3, alpha: pipeP });
    const tb = [110, 350, 590];
    tb.forEach((bx, i) => {
      const p = easeOut(span(lt, t3 + 1.0 + i * 0.14, t3 + 1.5 + i * 0.14));
      panel(bx, 608, 210, 92, {
        r: 10,
        fill: "rgba(10,14,28,0.7)",
        stroke: withAlpha(C.mint, i === 2 ? 0.8 : 0.25),
        alpha: pipeP * p,
      });
      text(i === 0 ? "EXEC" : i === 1 ? "DECODE" : "FETCH", bx + 18, 644, {
        size: T.chrome,
        family: F.mono,
        color: C.mint,
        alpha: pipeP * p,
      });
      text("16-bit", bx + 18, 676, { size: T.chrome, family: F.mono, color: C.dim, alpha: pipeP * p });
      text(["0x08000000", "0x08000002", "0x08000004"][i], bx + 18, 724, {
        size: T.chrome,
        family: F.mono,
        color: C.faint,
        alpha: pipeP * p,
      });
      if (i < 2) arrow(bx + 218, 654, bx + 232, 654, withAlpha(C.ink, 0.3), 1.2, 6, pipeP * p);
    });
    const tpcX = 695;
    arrow(tpcX, 576, tpcX, 605, C.mint, 1.6, 8, pipeP);
    text("PC", tpcX + 14, 582, { size: T.label, family: F.mono, color: C.mint, alpha: pipeP });
    text("= 0x08000004", tpcX + 14, 604, { size: T.chrome, family: F.mono, color: C.dim, alpha: pipeP });

    text("The pipeline is part of the instruction set.", 110, 806, {
      size: T.body,
      color: C.ink,
      alpha: pipeP,
    });
  }
}

// ---------------------------------------------------------------- ppu

function ppuScene(lt, sc) {
  const s = times(sc);
  const t0 = s[0].start;
  const t1 = s[1].start;
  const t2 = s[2].start;
  const t3 = s[3].start;

  head(sc, lt);

  const fx = 110;
  const fy = 200;
  const fw = 960;
  const fh = 640;
  let reveal = 0;
  if (lt < t1) reveal = lerp(0, 0.34, easeInOut(span(lt, t0 + 0.25, t1)));
  else if (lt < t2) reveal = lerp(0.34, 0.7, easeInOut(span(lt, t1, t2)));
  else if (lt < t3) reveal = lerp(0.7, 0.93, easeInOut(span(lt, t2, t3)));
  else reveal = lerp(0.93, 1, easeInOut(span(lt, t3, t3 + 1.6)));
  const lines = Math.floor(reveal * FB.h);
  const beamY = fy + (lines / FB.h) * fh;

  panel(fx - 8, fy - 8, fw + 16, fh + 16, {
    r: 14,
    fill: "rgba(4,6,14,0.9)",
    stroke: withAlpha(C.ink, 0.12),
    alpha: smooth(span(lt, t0, t0 + 0.8)),
  });
  scanImage("demo", fx, fy, fw, fh, lines, 1, 8);

  if (reveal < 1) {
    // the unlit part of the screen
    ctx.save();
    ctx.fillStyle = "rgba(4,6,14,0.9)";
    ctx.fillRect(fx, beamY, fw, fh - (beamY - fy));
    ctx.restore();
    // scan beam
    const a = smooth(span(lt, t0 + 0.1, t0 + 0.6)) * (1 - smooth(span(lt, t3 + 1.4, t3 + 2.0)));
    ctx.save();
    ctx.globalAlpha *= a;
    const g = ctx.createLinearGradient(0, beamY - 34, 0, beamY + 4);
    g.addColorStop(0, withAlpha(C.coral, 0));
    g.addColorStop(1, withAlpha(C.coral, 0.28));
    ctx.fillStyle = g;
    ctx.fillRect(fx, beamY - 34, fw, 38);
    ctx.fillStyle = withAlpha(C.coral, 0.9);
    ctx.fillRect(fx, beamY - 1, fw, 2);
    glow(fx + fw * 0.5, beamY, 160, C.coral, 0.12);
    ctx.restore();
  }

  // screen label
  text("SCANLINE", fx, fy - 22, { size: T.chrome, family: F.mono, color: C.dim, ls: 2, alpha: smooth(span(lt, t0 + 0.3, t0 + 0.8)) });
  text(`${String(Math.min(lines, 160)).padStart(3, "0")} / 160`, fx + 100, fy - 22, {
    size: T.chrome,
    family: F.mono,
    color: C.coral,
    alpha: smooth(span(lt, t0 + 0.3, t0 + 0.8)),
  });
  text("CINNABAR LOGO / LIVE FRAMEBUFFER", fx + fw, fy - 22, {
    size: T.chrome,
    family: F.mono,
    color: C.faint,
    align: "right",
    alpha: smooth(span(lt, t0 + 0.3, t0 + 0.8)),
  });

  // right-hand panel
  const rx = 1120;
  const ry = 200;
  const rw = 690;
  panel(rx, ry, rw, fh, { r: 16, fill: "rgba(9,13,26,0.55)", stroke: "rgba(244,241,234,0.09)", alpha: smooth(span(lt, t1 - 0.2, t1 + 0.5)) });

  // layer stack
  const layerP = smooth(span(lt, t1 - 0.1, t1 + 0.6)) * (1 - smooth(span(lt, t2 - 0.2, t2 + 0.2)));
  if (layerP > 0) {
    label("THE LINE ASKS", rx + 26, ry + 40, { color: C.peach, alpha: layerP });
    const layers = [
      { n: "BG0", t: "text", c: C.peach, pattern: "grid" },
      { n: "BG1", t: "text", c: C.mint, pattern: "grid" },
      { n: "BG2", t: "affine", c: C.cyan, pattern: "affine" },
      { n: "BG3", t: "affine", c: C.violet, pattern: "affine" },
      { n: "OBJ", t: "128 sprites", c: C.coral, pattern: "obj" },
    ];
    layers.forEach((l, i) => {
      const p = easeOut(span(lt, t1 + 0.2 + i * 0.18, t1 + 0.65 + i * 0.18));
      const y = ry + 76 + i * 88;
      panel(rx + 26, y, rw - 52, 70, {
        r: 10,
        fill: "rgba(10,14,28,0.66)",
        stroke: withAlpha(l.c, 0.24),
        alpha: layerP * p,
      });
      // mini pattern
      const mx = rx + 44;
      const my = y + 14;
      if (l.pattern === "grid") {
        for (let gy = 0; gy < 3; gy++) {
          for (let gx = 0; gx < 4; gx++) {
            rrect(mx + gx * 11, my + gy * 14, 10, 13, 1);
            ctx.fillStyle = withAlpha(l.c, 0.18 + ((gx + gy) % 2) * 0.2);
            ctx.globalAlpha = layerP * p;
            ctx.fill();
            ctx.globalAlpha = 1;
          }
        }
      } else if (l.pattern === "affine") {
        ctx.save();
        ctx.translate(mx + 22, my + 20);
        ctx.rotate(0.35);
        for (let gy = -1; gy < 2; gy++) {
          for (let gx = -1; gx < 2; gx++) {
            rrect(gx * 14, gy * 14, 12, 12, 1);
            ctx.fillStyle = withAlpha(l.c, 0.24);
            ctx.globalAlpha = layerP * p;
            ctx.fill();
            ctx.globalAlpha = 1;
          }
        }
        ctx.restore();
      } else {
        for (let j = 0; j < 6; j++) {
          dot(mx + 10 + (j % 3) * 16, my + 12 + Math.floor(j / 3) * 18, 5, l.c, layerP * p * 0.8);
        }
      }
      text(l.n, rx + 150, y + 32, { size: T.caption, family: F.mono, color: l.c, ls: 2, alpha: layerP * p });
      text(l.t, rx + 238, y + 32, { size: T.caption, color: C.ink, alpha: layerP * p });
    });
    text("4 backgrounds + 128 sprites, every line", rx + 26, ry + 580, {
      size: T.label,
      family: F.mono,
      color: C.dim,
      alpha: layerP,
    });
    text("modes 0-5", rx + rw - 26, ry + 580, { size: T.chrome, family: F.mono, color: C.faint, align: "right", alpha: layerP });
  }

  // blend panel
  const blendP = smooth(span(lt, t2 - 0.1, t2 + 0.6)) * (1 - smooth(span(lt, t3 - 0.2, t3 + 0.3)));
  if (blendP > 0) {
    label("THEN IT BLENDS", rx + 26, ry + 40, { color: C.coral, alpha: blendP });
    // two squares
    const sx = rx + 70;
    const sy = ry + 96;
    rrect(sx, sy, 170, 170, 12);
    ctx.fillStyle = withAlpha(C.coral, 0.75);
    ctx.fill();
    rrect(sx + 90, sy + 50, 170, 170, 12);
    ctx.fillStyle = withAlpha(C.cyan, 0.6);
    ctx.fill();
    text("A", sx + 26, sy + 132, { size: T.title, weight: 600, color: C.ink, alpha: blendP });
    text("B", sx + 116, sy + 182, { size: T.title, weight: 600, color: C.ink, alpha: blendP });
    // equation
    text("A x alpha + B x (1 - alpha)", rx + 26, ry + 370, {
      size: T.caption,
      family: F.mono,
      color: C.ink,
      alpha: blendP,
    });
    // slider
    const sl = rx + 26;
    const sy2 = ry + 410;
    line(sl, sy2, sl + 400, sy2, withAlpha(C.ink, 0.2), 2, null, blendP);
    const val = 0.55 + Math.sin(lt * 0.7) * 0.2;
    dot(sl + 400 * val, sy2, 7, C.coral, blendP, 10);
    text(`${Math.round(val * 100)}%`, sl + 420, sy2 + 6, { size: T.label, family: F.mono, color: C.coral, alpha: blendP });
    const effects = ["ALPHA", "BRIGHTNESS", "WINDOW", "MOSAIC"];
    effects.forEach((e, i) => {
      const p = smooth(span(lt, t2 + 0.05 + i * 0.45, t2 + 0.4 + i * 0.45));
      pill(e, rx + 26, ry + 500 + i * 44, i === 0 ? C.coral : C.peach, { alpha: p * blendP });
    });
  }

  // timing panel
  const timeP = smooth(span(lt, t3 - 0.1, t3 + 0.6));
  if (timeP > 0) {
    label("ONE FRAME", rx + 26, ry + 40, { color: C.mint, alpha: timeP });
    const bx = rx + 40;
    const by = ry + 80;
    for (let i = 0; i < 228; i++) {
      const visible = i < 160;
      const p = smooth(span(lt, t3 + 0.1 + i * 0.002, t3 + 0.3 + i * 0.002));
      line(bx, by + i * 2, bx + (visible ? 300 : 190), by + i * 2, withAlpha(visible ? C.mint : C.faint, visible ? 0.55 : 0.2), 1, null, timeP * p);
    }
    bracket(bx + 320, bx + 340, by, "", C.mint, 0);
    text("160", bx + 356, by + 170, { size: T.caption, family: F.mono, color: C.mint, alpha: timeP });
    text("VISIBLE", bx + 356, by + 196, { size: T.chrome, family: F.mono, color: C.dim, alpha: timeP });
    text("68", bx + 356, by + 356, { size: T.caption, family: F.mono, color: C.faint, alpha: timeP });
    text("VBLANK", bx + 356, by + 382, { size: T.chrome, family: F.mono, color: C.faint, alpha: timeP });
    text("228", rx + rw - 30, by + 140, { size: T.title, family: F.mono, color: C.ink, align: "right", alpha: timeP });
    text("SCANLINES", rx + rw - 30, by + 174, { size: T.chrome, family: F.mono, color: C.dim, align: "right", alpha: timeP });
    text("59.7275 Hz", rx + rw - 30, by + 240, { size: T.caption, family: F.mono, color: C.coral, align: "right", alpha: timeP });
    text("top to bottom", rx + rw - 30, by + 272, { size: T.chrome, family: F.mono, color: C.faint, align: "right", alpha: timeP });
    if (reveal >= 1) {
      const done = smooth(span(lt, t3 + 1.8, t3 + 2.4));
      pill("FRAME READY", rx + 26, ry + 580, C.mint, { alpha: done * timeP });
    }
  }
}

// ---------------------------------------------------------------- apu

function apuScene(lt, sc) {
  const s = times(sc);
  const t0 = s[0].start;
  const t1 = s[1].start;
  const t2 = s[2].start;
  const t3 = s[3].start;

  head(sc, lt);

  const rows = [
    { tag: "CH1", name: "SQUARE 1", kind: "square", color: C.coral, duty: 0.5, on: t0 + 0.1 },
    { tag: "CH2", name: "SQUARE 2", kind: "square", color: C.peach, duty: 0.25, on: t0 + 0.7 },
    { tag: "CH3", name: "WAVE", kind: "wave", color: C.mint, on: t0 + 1.5 },
    { tag: "CH4", name: "NOISE", kind: "noise", color: C.violet, on: t0 + 2.3 },
    { tag: "DMA A", name: "DMA SOUND A", kind: "wave", color: C.cyan, dma: true, on: t1 + 0.1 },
    { tag: "DMA B", name: "DMA SOUND B", kind: "noise", color: C.cyan, dma: true, on: t1 + 0.7 },
  ];

  rows.forEach((r, i) => {
    const p = easeOut(span(lt, r.on, r.on + 0.7));
    if (p <= 0) return;
    const y = 176 + i * 104;
    const dim = r.dma ? 0.8 : 1;
    panel(110, y, 1100, 82, {
      r: 10,
      fill: r.dma ? "rgba(9,20,24,0.55)" : "rgba(9,13,26,0.66)",
      stroke: withAlpha(r.color, r.dma ? 0.2 : 0.24),
      alpha: p,
    });
    text(r.tag, 132, y + 34, { size: T.caption, family: F.mono, color: r.color, ls: 1.5, alpha: p });
    text(r.name, 132, y + 60, { size: T.chrome, family: F.mono, color: C.dim, alpha: p });
    // waveform
    const wx = 340;
    const wy = y + 14;
    const ww = 700;
    const wh = 54;
    if (r.kind === "square") drawSquare(wx, wy, ww, wh, lt * 0.9 + i * 0.13, r.duty, r.color, 2.0, p * dim);
    else if (r.kind === "wave") drawStepWave(wx, wy, ww, wh, lt * 0.6 + i * 0.1, r.color, 2.0, p * dim);
    else drawNoise(wx, wy, ww, wh, lt * 0.9 + i * 0.2, r.color, 1.4, p * dim);
    // level
    const level = 0.45 + 0.35 * Math.sin(lt * 2.1 + i * 1.3) * Math.sin(lt * 0.7 + i);
    meter(1066, y + 32, 110, 8, level, r.color, p * dim);
    if (r.dma) {
      label("STREAM", 1066, y + 68, { size: T.chrome, color: C.cyan, alpha: p * 0.7 });
    }
  });

  // mixer panel
  const mx = 1260;
  const my = 230;
  const mp = easeOut(span(lt, t2 - 0.1, t2 + 0.9));
  if (mp > 0) {
    panel(mx, my, 550, 540, {
      r: 16,
      fill: "rgba(10,14,28,0.68)",
      stroke: withAlpha(C.mint, 0.22),
      alpha: mp,
    });
    label("MIXER", mx + 24, my + 36, { color: C.mint, alpha: mp });

    // six channel faders
    const faderY = my + 86;
    const faderH = 128;
    for (let i = 0; i < 6; i++) {
      const fx = mx + 100 + i * 62;
      line(fx, faderY, fx, faderY + faderH, withAlpha(C.ink, 0.14), 2, null, mp);
      const level = 0.35 + 0.3 * Math.abs(Math.sin(lt * 0.9 + i * 1.1));
      const ky = faderY + faderH * (1 - level);
      dot(fx, ky, 7, rows[i].color, mp, 8);
      label(["CH1", "CH2", "CH3", "CH4", "DMA1", "DMA2"][i], fx, faderY + faderH + 24, {
        size: T.chrome,
        color: rows[i].color,
        align: "center",
        alpha: mp * 0.8,
      });
    }
    text("SIX CHANNELS", mx + 24, my + 300, { size: T.chrome, family: F.mono, color: C.dim, ls: 2, alpha: mp });
    line(mx + 24, my + 322, mx + 526, my + 322, withAlpha(C.ink, 0.12), 1, null, mp);
    // output
    text("48 kHz", mx + 24, my + 396, { size: T.title, family: F.mono, color: C.ink, alpha: mp });
    text("STEREO", mx + 24, my + 434, { size: T.caption, family: F.mono, color: C.mint, ls: 2, alpha: mp });
    // L/R meters
    ["L", "R"].forEach((ch, i) => {
      text(ch, mx + 24 + i * 170, my + 490, { size: T.label, family: F.mono, color: C.dim, alpha: mp });
      const level = 0.55 + 0.25 * Math.sin(lt * 1.7 + i * Math.PI);
      meter(mx + 54 + i * 170, my + 474, 110, 10, level, i ? C.cyan : C.mint, mp);
    });
    text("48000 samples / second", mx + 24, my + 524, { size: T.chrome, family: F.mono, color: C.faint, alpha: mp });
  }

  // envelope + sweep
  const ep = smooth(span(lt, t3 - 0.1, t3 + 0.6));
  if (ep > 0) {
    panel(110, 820, 560, 160, {
      r: 14,
      fill: "rgba(10,14,28,0.6)",
      stroke: withAlpha(C.coral, 0.22),
      alpha: ep,
    });
    label("ENVELOPE / ADSR", 134, 852, { color: C.coral, alpha: ep });
    ctx.save();
    ctx.globalAlpha *= ep;
    ctx.strokeStyle = C.coral;
    ctx.lineWidth = 2;
    ctx.beginPath();
    ctx.moveTo(140, 954);
    ctx.lineTo(218, 878);
    ctx.lineTo(306, 908);
    ctx.lineTo(436, 908);
    ctx.lineTo(560, 954);
    ctx.stroke();
    ctx.restore();
    ["A", "D", "S", "R"].forEach((l, i) => {
      const lx = [178, 262, 371, 498][i];
      text(l, lx, 972, { size: T.chrome, family: F.mono, color: C.coral, align: "center", alpha: ep });
    });
    // sweep arrow on CH2
    arrow(1110, 300, 1190, 274, withAlpha(C.peach, 0.7), 1.4, 8, ep);
    text("SWEEP", 1116, 336, { size: T.chrome, family: F.mono, color: C.peach, alpha: ep });
    pill("FRAME SEQ 512 Hz", 730, 930, C.mint, { alpha: ep });
  }
}

// ---------------------------------------------------------------- bios

function biosScene(lt, sc) {
  const s = times(sc);
  const t0 = s[0].start;
  const t1 = s[1].start;
  const t2 = s[2].start;
  const t3 = s[3].start;

  head(sc, lt);

  const bootP = easeOut(span(lt, t0 + 0.1, t0 + 0.8));
  const skipP = smooth(span(lt, t1 - 0.2, t1 + 0.5));
  const swiP = easeOut(span(lt, t2 - 0.1, t2 + 0.7));
  const cleanP = smooth(span(lt, t3 - 0.2, t3 + 0.5));

  // boot path
  if (bootP > 0) {
    const y = 210;
    panel(110, y, 200, 100, { r: 12, stroke: withAlpha(C.ink, 0.16), alpha: bootP });
    text("POWER", 134, y + 40, { size: T.caption, family: F.mono, color: C.ink, ls: 2, alpha: bootP });
    text("on", 134, y + 70, { size: T.chrome, family: F.mono, color: C.faint, alpha: bootP });

    arrow(318, y + 50, 388, y + 50, withAlpha(C.ink, 0.35), 1.4, 8, bootP);

    const bioA = bootP * (1 - 0.55 * skipP);
    panel(400, y, 280, 100, {
      r: 12,
      fill: "rgba(20,12,18,0.6)",
      stroke: withAlpha(C.coral, 0.4),
      alpha: bioA,
    });
    text("BIOS", 424, y + 40, { size: T.caption, family: F.mono, color: C.coral, ls: 2, alpha: bioA });
    text("16 KB / Nintendo", 424, y + 70, { size: T.chrome, family: F.mono, color: C.dim, alpha: bioA });
    if (skipP > 0.2) {
      line(424, y + 54, 656, y + 18, withAlpha(C.coral, 0.8), 2.2, null, skipP);
      label("SKIPPED", 540, y + 88, { size: T.chrome, color: C.coral, align: "center", alpha: skipP });
    }

    arrow(690, y + 50, 760, y + 50, withAlpha(C.ink, 0.35), 1.4, 8, bootP);

    panel(770, y, 240, 100, { r: 12, stroke: withAlpha(C.mint, 0.3), alpha: bootP });
    text("GAME", 794, y + 40, { size: T.caption, family: F.mono, color: C.mint, ls: 2, alpha: bootP });
    text("cartridge entry", 794, y + 70, { size: T.chrome, family: F.mono, color: C.dim, alpha: bootP });

    // bypass
    if (skipP > 0) {
      curve(
        [
          [210, y + 108],
          [210, y + 150],
          [890, y + 150],
          [890, y + 108],
        ],
        withAlpha(C.mint, 0.55),
        1.6,
        skipP,
      );
      arrow(890, y + 130, 890, y + 104, withAlpha(C.mint, 0.55), 1.6, 8, skipP);
      text("HLE: jump straight in", 550, y + 180, { size: T.chrome, family: F.mono, color: C.mint, alpha: skipP });
    }
  }

  // no firmware card
  if (bootP > 0) {
    panel(1120, 190, 690, 210, {
      r: 14,
      fill: "rgba(10,14,28,0.62)",
      stroke: withAlpha(cleanP > 0 ? C.mint : C.ink, cleanP > 0 ? 0.4 : 0.12),
      alpha: bootP,
    });
    label("WHAT SHIPS", 1144, 224, { color: cleanP > 0 ? C.mint : C.dim, alpha: bootP });
    text("bios.bin", 1144, 282, { size: T.caption, family: F.mono, color: C.ink, alpha: bootP });
    if (skipP > 0) {
      cross(1300, 272, skipP, C.coral, 12);
      text("not in the repo", 1330, 282, { size: T.label, family: F.mono, color: C.coral, alpha: skipP });
    }
    text("the cartridge still boots", 1144, 336, { size: T.label, color: C.dim, alpha: cleanP });
    if (cleanP > 0) {
      check(1500, 328, cleanP, C.mint);
    }
    label("CLEAN ROOM / MIT", 1144, 372, { color: C.faint, alpha: cleanP });
  }

  // SWI table
  if (swiP > 0) {
    label("SWI DISPATCH", 110, 464, { color: C.mint, alpha: swiP });
    const rows = [
      { imm: "0x12", name: "LZ77UnCompWram", verb: "decompress", color: C.coral, at: 0.2 },
      { imm: "0xC0", name: "CpuSet", verb: "fill", color: C.peach, at: 1.7 },
      { imm: "0x06", name: "Div", verb: "divide", color: C.cyan, at: 3.3 },
      { imm: "0x05", name: "VBlankIntrWait", verb: "wait", color: C.mint, at: 4.9 },
    ];
    rows.forEach((r, i) => {
      const p = easeOut(span(lt, t2 + r.at, t2 + r.at + 0.6));
      const y = 494 + i * 92;
      panel(110, y, 1050, 78, {
        r: 10,
        fill: withAlpha(r.color, 0.07),
        stroke: withAlpha(r.color, 0.22),
        alpha: swiP * p,
      });
      text(r.imm, 130, y + 46, { size: T.caption, family: F.mono, color: r.color, alpha: swiP * p });
      text(r.name, 250, y + 46, { size: T.caption, family: F.mono, color: C.ink, alpha: swiP * p });
      text(r.verb, 790, y + 46, { size: T.label, family: F.mono, color: r.color, align: "right", alpha: swiP * p });
      arrow(830, y + 39, 900, y + 39, withAlpha(C.ink, 0.25), 1.2, 7, swiP * p);
      text("Rust", 950, y + 46, { size: T.label, family: F.mono, color: C.mint, alpha: swiP * p });
    });
  }

  // code card + dispatch
  if (swiP > 0) {
    const cx = 1210;
    const cy = 470;
    panel(cx, cy, 600, 236, {
      r: 14,
      fill: "rgba(6,9,18,0.86)",
      stroke: "rgba(244,241,234,0.11)",
      alpha: swiP,
    });
    label("THE CALL", cx + 22, cy + 34, { color: C.dim, alpha: swiP });
    const activeCall = lt >= t2 + 4.9 ? "swi #0x05" : lt >= t2 + 3.3 ? "swi #0x06" : lt >= t2 + 1.7 ? "swi #0xC0" : "swi #0x12";
    text(activeCall, cx + 22, cy + 86, { size: T.caption, family: F.mono, color: C.ink, alpha: swiP });
    text("bios.rs", cx + 22, cy + 124, { size: T.chrome, family: F.mono, color: C.faint, alpha: swiP });
    text("fn swi(imm) { match imm { ... } }", cx + 22, cy + 164, { size: T.label, family: F.mono, color: C.mint, alpha: swiP });
    text("no firmware image required", cx + 22, cy + 204, { size: T.chrome, family: F.mono, color: C.dim, alpha: swiP });

    panel(cx, 740, 600, 106, {
      r: 14,
      fill: "rgba(10,14,28,0.6)",
      stroke: withAlpha(C.mint, 0.24),
      alpha: swiP,
    });
    label("THE RESULT", cx + 22, 774, { color: C.mint, alpha: swiP });
    text("real cartridges boot", cx + 22, 812, { size: T.label, color: C.ink, alpha: swiP });
    text("MIT / no copyrighted firmware", cx + 22, 838, { size: T.chrome, family: F.mono, color: C.faint, alpha: swiP });
  }
}

// ---------------------------------------------------------------- proof

function proofScene(lt, sc) {
  const s = times(sc);
  const t0 = s[0].start;
  const t1 = s[1].start;
  const t2 = s[2].start;
  const t3 = s[3].start;

  head(sc, lt);

  const termP = easeOut(span(lt, t0 + 0.2, t0 + 0.9));
  const tx = 110;
  const ty = 176;
  const tw = 980;
  const th = 700;

  panel(tx, ty, tw, th, {
    r: 16,
    fill: "rgba(6,9,18,0.9)",
    stroke: "rgba(244,241,234,0.11)",
    alpha: termP,
  });
  text("● ● ●", tx + 22, ty + 32, { size: T.chrome, family: F.mono, color: C.faint, alpha: termP });
  text("tools/validate.sh", tx + tw - 22, ty + 32, { size: T.chrome, family: F.mono, color: C.faint, align: "right", alpha: termP });
  line(tx, ty + 48, tx + tw, ty + 48, "rgba(244,241,234,0.08)", 1, null, termP);

  text("$ ./tools/validate.sh", tx + 26, ty + 90, { size: T.label, family: F.mono, color: C.ink, alpha: termP });

  const lines = [
    ["PASS", "arm", "r12=00000000"],
    ["PASS", "thumb", "r7=00000000"],
    ["PASS", "memory", "r12=00000000"],
    ["PASS", "sram", "r12=00000000"],
    ["PASS", "flash64", "r12=00000000"],
    ["PASS", "flash128", "r12=00000000"],
    ["PASS", "celeste", "colors=11  audio_peak=0.5000"],
  ];
  lines.forEach((ln, i) => {
    const p = easeOut(span(lt, t1 + 0.1 + i * 0.55, t1 + 0.55 + i * 0.55));
    if (p <= 0) return;
    const y = ty + 150 + i * 54;
    const ok = smooth(span(lt, t2 - 0.1 + i * 0.06, t2 + 0.35 + i * 0.06));
    text(ln[0], tx + 26, y, { size: T.caption, family: F.mono, color: ok > 0.5 ? C.mint : C.dim, alpha: p * termP });
    text(ln[1], tx + 130, y, { size: T.caption, family: F.mono, color: C.ink, alpha: p * termP });
    text(ln[2], tx + 330, y, { size: T.chrome, family: F.mono, color: C.faint, alpha: p * termP });
    if (ok > 0) check(tx + 900, y - 8, ok, C.mint);
  });

  const allP = smooth(span(lt, t2 + 0.5, t2 + 1.1));
  if (allP > 0) {
    line(tx + 26, ty + 550, tx + tw - 26, ty + 550, "rgba(244,241,234,0.1)", 1, null, allP);
    text("all selected validations passed", tx + 26, ty + 594, {
      size: T.body,
      family: F.mono,
      color: C.mint,
      alpha: allP * termP,
    });
    text("CPU, memory and save suites, plus the real-game smoke test", tx + 26, ty + 644, {
      size: T.chrome,
      family: F.mono,
      color: C.faint,
      alpha: allP * termP,
    });
  }

  // suite grid
  const gridP = easeOut(span(lt, t1 - 0.1, t1 + 0.6));
  const gx = 1150;
  const gy = 176;
  const suites = ["ARM", "THUMB", "MEMORY", "SRAM", "FLASH 64K", "FLASH 128K"];
  suites.forEach((name, i) => {
    const col = i % 3;
    const row = Math.floor(i / 3);
    const x = gx + col * 224;
    const y = gy + row * 116;
    const p = easeOut(span(lt, t1 + 0.2 + i * 0.12, t1 + 0.65 + i * 0.12));
    const ok = smooth(span(lt, t2 + 0.05 + i * 0.1, t2 + 0.4 + i * 0.1));
    const passed = ok > 0.35;
    panel(x, y, 204, 96, {
      r: 10,
      fill: passed ? "rgba(12,32,28,0.55)" : "rgba(10,14,28,0.62)",
      stroke: passed ? withAlpha(C.mint, 0.4) : "rgba(244,241,234,0.1)",
      alpha: gridP * p,
    });
    label(name, x + 18, y + 32, { size: T.chrome, color: passed ? C.mint : C.dim, alpha: gridP * p });
    text(passed ? "PASS" : "waiting", x + 18, y + 68, {
      size: T.label,
      family: F.mono,
      color: passed ? C.mint : C.faint,
      alpha: gridP * p,
    });
    if (passed) check(x + 166, y + 60, ok, C.mint);
  });

  // performance panel
  const perfP = easeOut(span(lt, t3 - 0.1, t3 + 0.7));
  if (perfP > 0) {
    const px = 1150;
    const py = 448;
    panel(px, py, 660, 428, {
      r: 16,
      fill: "rgba(10,14,28,0.66)",
      stroke: withAlpha(C.coral, 0.28),
      alpha: perfP,
    });
    label("HEADLESS / CELESTE CLASSIC", px + 26, py + 38, { color: C.coral, alpha: perfP });
    const n = 429.7 * easeOut(span(lt, t3 + 0.1, t3 + 2.8));
    text(n.toFixed(1), px + 24, py + 142, { size: T.display, weight: 500, family: F.mono, color: C.ink, alpha: perfP });
    text("fps", px + 26, py + 186, { size: T.label, family: F.mono, color: C.dim, alpha: perfP });
    text("bench: 300 frames in 0.698 s", px + 24, py + 224, { size: T.chrome, family: F.mono, color: C.faint, alpha: perfP });
    imageCrop("celeste", 56, 16, 128, 128, px + 450, py + 58, 150, 150, perfP, 8);
    text("CELESTE / REAL GAME", px + 450, py + 232, { size: T.chrome, family: F.mono, color: C.dim, alpha: perfP });
    // bar
    const bx = px + 26;
    const by = py + 274;
    const bw = 600;
    rrect(bx, by, bw, 12, 6);
    ctx.fillStyle = "rgba(244,241,234,0.08)";
    ctx.fill();
    rrect(bx, by, bw * (n / 430), 12, 6);
    ctx.fillStyle = C.coral;
    ctx.fill();
    // required marker
    const reqX = bx + bw * (59.7275 / 430);
    line(reqX, by - 16, reqX, by + 32, C.mint, 2);
    text("60", reqX, by + 54, { size: T.chrome, family: F.mono, color: C.mint, align: "center", alpha: perfP });
    text("430", bx + bw, by + 54, { size: T.chrome, family: F.mono, color: C.faint, align: "right", alpha: perfP });
    text("7.2x headroom", bx, by + 92, { size: T.caption, family: F.mono, color: C.mint, alpha: perfP });
    text("the frontend only needs 59.7275", bx, by + 126, { size: T.chrome, family: F.mono, color: C.dim, alpha: perfP });
  }
}

// ---------------------------------------------------------------- browser

function browserScene(lt, sc) {
  const s = times(sc);
  const t0 = s[0].start;
  const t1 = s[1].start;
  const t2 = s[2].start;
  const t3 = s[3].start;

  head(sc, lt);

  const coreP = easeOut(span(lt, t0, t0 + 0.8));
  const noP = smooth(span(lt, t0 + 0.2, t0 + 0.9)) * (1 - smooth(span(lt, t1 - 0.6, t1 - 0.1)));
  const frontP = easeOut(span(lt, t1 - 0.1, t1 + 0.8));
  const shotP = easeOut(span(lt, t2 - 0.2, t2 + 0.9));

  const core = { x: 760, y: 140, w: 400, h: 120 };
  panel(core.x, core.y, core.w, core.h, {
    r: 16,
    fill: "rgba(18,12,20,0.7)",
    stroke: withAlpha(C.coral, 0.5),
    alpha: coreP,
    glow: withAlpha(C.coral, 0.09),
  });
  text("cinnabar-core", core.x + 24, core.y + 48, { size: T.caption, family: F.mono, color: C.ink, alpha: coreP });
  text("no window, no audio device, no filesystem", core.x + 24, core.y + 84, {
    size: T.chrome,
    family: F.mono,
    color: C.dim,
    alpha: coreP,
  });
  label("THE CORE", core.x + core.w - 24, core.y + 36, { color: C.coral, align: "right", alpha: coreP });

  // three "no" cards
  if (noP > 0) {
    const items = ["NO WINDOW", "NO AUDIO DEVICE", "NO FILESYSTEM"];
    items.forEach((it, i) => {
      const p = easeOut(span(lt, t0 + 0.3 + i * 1.0, t0 + 0.9 + i * 1.0));
      const x = 300 + i * 460;
      panel(x, 300, 380, 72, {
        r: 10,
        fill: "rgba(10,14,28,0.6)",
        stroke: "rgba(244,241,234,0.12)",
        alpha: p * noP,
      });
      label(it, x + 22, 330, { color: C.dim, alpha: p * noP });
      text("the core owns none of it", x + 22, 354, { size: T.chrome, family: F.mono, color: C.faint, alpha: p * noP });
      line(x + 20, 338, x + 360, 338, withAlpha(C.coral, 0.6), 1.6, null, p * noP);
    });
  }

  // frontends
  if (frontP > 0) {
    arrow(core.x + 100, core.y + 128, 560, 304, withAlpha(C.ink, 0.35), 1.4, 8, frontP);
    arrow(core.x + 300, core.y + 128, 1360, 304, withAlpha(C.ink, 0.35), 1.4, 8, frontP);
    const cards = [
      { x: 300, title: "NATIVE", sub: "winit / softbuffer / cpal", line1: "a desktop window", color: C.peach },
      { x: 1100, title: "BROWSER", sub: "WebAssembly / Canvas / AudioWorklet", line1: "a browser tab", color: C.mint },
    ];
    cards.forEach((c) => {
      panel(c.x, 312, 520, 140, {
        r: 14,
        fill: "rgba(10,14,28,0.66)",
        stroke: withAlpha(c.color, 0.32),
        alpha: frontP,
      });
      text(c.title, c.x + 24, 356, { size: T.caption, family: F.mono, color: c.color, ls: 2, alpha: frontP });
      text(c.sub, c.x + 24, 400, { size: T.label, family: F.mono, color: C.ink, alpha: frontP });
      text(c.line1, c.x + 24, 432, { size: T.chrome, family: F.mono, color: C.dim, alpha: frontP });
    });
  }

  // real arcade screenshot
  if (shotP > 0) {
    const ix = 110;
    const iy = 500;
    const iw = 665;
    const ih = 460;
    ctx.save();
    ctx.globalAlpha *= shotP;
    glow(ix + iw / 2, iy + ih / 2, 500, C.coral, 0.1);
    ctx.restore();
    panel(ix - 7, iy - 7, iw + 14, ih + 14, {
      r: 14,
      fill: "rgba(4,6,14,0.85)",
      stroke: withAlpha(C.ink, 0.14),
      alpha: shotP,
    });
    imageCrop("web", 165, 580, 1105, 765, ix, iy, iw, ih, shotP, 8);
    label("CINNABAR ARCADE / REAL CAPTURE", ix, iy - 22, { color: C.dim, alpha: shotP });
    text("play in a tab, no install", ix + iw, iy - 22, { size: T.chrome, family: F.mono, color: C.faint, align: "right", alpha: shotP });
  }

  // browser feature rows
  const featP = smooth(span(lt, t2 + 0.1, t2 + 0.8));
  if (featP > 0) {
    const fx = 850;
    const fy = 510;
    const feats = [
      ["AUDIOWORKLET", "48 kHz sound, scheduled off the main thread"],
      ["INDEXEDDB", "SRAM, Flash and EEPROM saves persist"],
      ["WEBGL2", "LCD and CRT shaders on the framebuffer"],
      ["TOUCH + GAMEPAD", "the same buttons, in the browser"],
    ];
    feats.forEach((f, i) => {
      const p = smooth(span(lt, t2 + 0.2 + i * 0.28, t2 + 0.6 + i * 0.28)) * featP;
      const y = fy + i * 78;
      text(f[0], fx, y, { size: T.chrome, family: F.mono, color: C.mint, ls: 2, alpha: p });
      text(f[1], fx, y + 28, { size: T.label, color: C.dim, alpha: p });
      line(fx, y + 48, fx + 840, y + 48, "rgba(244,241,234,0.08)", 1, null, p);
    });
    const carts = ["LOGO", "TILED", "INPUT", "IRQ", "HALT", "GRADIENT"];
    label("SIX BUILT-IN CARTRIDGES", fx, 838, { size: T.chrome, color: C.faint, alpha: featP });
    carts.forEach((c, i) => {
      const p = smooth(span(lt, t2 + 1.3 + i * 0.1, t2 + 1.6 + i * 0.1)) * featP;
      pill(c, fx + i * 138, 870, i === 0 ? C.coral : C.dim, { size: T.chrome, alpha: p });
    });
  }

  // one core, two frontends
  const oneP = smooth(span(lt, t3 - 0.2, t3 + 0.5));
  if (oneP > 0) {
    text("one core, two frontends", 850, 462, {
      size: T.body,
      family: F.serif,
      style: "italic",
      color: C.ink,
      alpha: oneP,
    });
    line(850, 478, 850 + 300 * oneP, 478, withAlpha(C.coral, 0.5), 1.4);
  }
}

// ---------------------------------------------------------------- close

function closeScene(lt, sc) {
  const s = times(sc);
  const t0 = s[0].start;
  const t1 = s[1].start;
  const t2 = s[2].start;
  const t3 = s[3].start;

  head(sc, lt, 1 - smooth(span(lt, t2, t2 + 1.0)));

  const zoom = 1.02 - 0.02 * easeInOut(span(lt, t0, t3));
  const fw = 960;
  const fh = 640;
  const fx = 480;
  const fy = 150;

  const finalP = smooth(span(lt, t3 - 0.2, t3 + 0.8));
  const dim = 1 - 0.82 * finalP;

  ctx.save();
  ctx.globalAlpha *= dim;
  glow(fx + fw / 2, fy + fh / 2, 620, C.coral, 0.12);
  panel(fx - 7, fy - 7, fw + 14, fh + 14, {
    r: 14,
    fill: "rgba(4,6,14,0.85)",
    stroke: withAlpha(C.ink, 0.14),
    alpha: smooth(span(lt, t0, t0 + 1.2)),
  });
  image("demo", fx, fy, fw, fh, smooth(span(lt, t0, t0 + 1.4)), 8, zoom);
  ctx.restore();

  // pixel grid and one pixel
  const gridP = smooth(span(lt, t1 - 0.1, t1 + 0.6)) * (1 - smooth(span(lt, t2 - 0.2, t2 + 0.4)));
  if (gridP > 0) {
    ctx.save();
    ctx.globalAlpha *= gridP * dim;
    ctx.strokeStyle = "rgba(244,241,234,0.09)";
    ctx.lineWidth = 1;
    for (let x = 0; x <= fw; x += 40) {
      line(fx + x, fy, fx + x, fy + fh, "rgba(244,241,234,0.09)", 1, null, 1);
    }
    for (let y = 0; y <= fh; y += 40) {
      line(fx, fy + y, fx + fw, fy + y, "rgba(244,241,234,0.09)", 1, null, 1);
    }
    ctx.restore();
    const mx = fx + 86 * ((fw * zoom) / 240);
    const my = fy + 80 * ((fh * zoom) / 160);
    ctx.save();
    ctx.globalAlpha *= gridP;
    ctx.fillStyle = C.ink;
    ctx.fillRect(mx - 2, my - 2, 4, 4);
    ctx.restore();
    ring(mx, my, 14, withAlpha(C.coral, 0.9), 1.6, gridP);
    line(mx + 16, my - 10, 1490, 280, withAlpha(C.coral, 0.5), 1, null, gridP);
    panel(1500, 214, 310, 130, {
      r: 12,
      fill: "rgba(10,14,28,0.7)",
      stroke: withAlpha(C.coral, 0.3),
      alpha: gridP,
    });
    label("ONE PIXEL", 1522, 248, { color: C.coral, alpha: gridP });
    text("38,400 in a frame", 1522, 292, { size: T.label, color: C.ink, alpha: gridP });
    text("240 x 160", 1522, 320, { size: T.chrome, family: F.mono, color: C.dim, alpha: gridP });
  }

  // no roms, no bios, no shortcuts
  const noP = smooth(span(lt, t2 - 0.1, t2 + 0.6)) * (1 - smooth(span(lt, t3 - 0.3, t3 + 0.5)));
  if (noP > 0) {
    const items = ["NO GAME ROMS", "NO BIOS", "NO SHORTCUTS"];
    items.forEach((it, i) => {
      const p = smooth(span(lt, t2 + 0.1 + i * 0.35, t2 + 0.5 + i * 0.35)) * noP;
      pill(it, 960 + (i - 1) * 300, 878, i === 2 ? C.coral : C.mint, { size: T.label, align: "center", alpha: p });
    });
  }

  // the landing
  if (finalP > 0) {
    const p = easeOut(span(lt, t3 + 0.1, t3 + 1.1));
    const scrim = ctx.createRadialGradient(960, 520, 0, 960, 520, 720);
    scrim.addColorStop(0, `rgba(7,10,21,${0.62 * p})`);
    scrim.addColorStop(0.55, `rgba(7,10,21,${0.34 * p})`);
    scrim.addColorStop(1, "rgba(7,10,21,0)");
    ctx.fillStyle = scrim;
    ctx.fillRect(0, 0, W, H);
    text("A game you love,", 960, 470, {
      size: T.title,
      family: F.serif,
      style: "italic",
      color: C.ink,
      align: "center",
      alpha: p,
    });
    text("running because of code you wrote.", 960, 560, {
      size: T.title,
      family: F.serif,
      style: "italic",
      color: C.ink,
      align: "center",
      alpha: p,
    });
    line(960 - 170 * p, 610, 960 + 170 * p, 610, withAlpha(C.coral, 0.7), 1.4);
  }
}

// ---------------------------------------------------------------- map

export const SCENES = {
  frame: frameScene,
  machine: machineScene,
  bus: busScene,
  cpu: cpuScene,
  ppu: ppuScene,
  apu: apuScene,
  bios: biosScene,
  proof: proofScene,
  browser: browserScene,
  close: closeScene,
};
