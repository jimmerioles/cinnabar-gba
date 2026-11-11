import init, { Emulator } from "./pkg/cinnabar_wasm.js";
import { AudioEngine } from "./audio.js";
import { SaveStore, hashRom } from "./db.js";
import { createRenderer } from "./renderer.js";

const GBA_FPS = 16_777_216 / 280_896;
const FRAME_MS = 1000 / GBA_FPS;
const MAX_CATCHUP_FRAMES = 4;
const FAST_FORWARD_FRAMES = 4;
const SETTINGS_KEY = "cinnabar-arcade";

const KEYS = Object.freeze({
  A: 1 << 0,
  B: 1 << 1,
  SELECT: 1 << 2,
  START: 1 << 3,
  RIGHT: 1 << 4,
  LEFT: 1 << 5,
  UP: 1 << 6,
  DOWN: 1 << 7,
  R: 1 << 8,
  L: 1 << 9,
});

const KEY_MAP = Object.freeze({
  KeyZ: KEYS.A,
  KeyA: KEYS.A,
  KeyX: KEYS.B,
  KeyS: KEYS.B,
  Enter: KEYS.START,
  Backspace: KEYS.SELECT,
  ArrowRight: KEYS.RIGHT,
  ArrowLeft: KEYS.LEFT,
  ArrowUp: KEYS.UP,
  ArrowDown: KEYS.DOWN,
  KeyQ: KEYS.L,
  KeyE: KEYS.R,
});

const BUILTINS = [
  {
    id: "logo",
    name: "Cinnabar logo",
    note: "Mode 3 · the mineral mark",
    code: "C",
  },
  {
    id: "input",
    name: "Input test",
    note: "Every button lights up",
    code: "I",
  },
  {
    id: "tiled",
    name: "Tiles & sprites",
    note: "Mode 0 · alpha blending",
    code: "T",
  },
  {
    id: "irq",
    name: "DMA & interrupts",
    note: "Timer + VBlank IRQ",
    code: "D",
  },
  {
    id: "halt",
    name: "Halt & wake",
    note: "VBlankIntrWait",
    code: "H",
  },
  {
    id: "gradient",
    name: "Gradient",
    note: "Mode 3 test pattern",
    code: "G",
  },
];

const DEFAULT_SETTINGS = {
  volume: 0.75,
  muted: false,
  shader: "lcd",
  rememberRom: false,
  lastHash: null,
  hud: false,
};

await bootWasm();

class Arcade {
  constructor() {
    this.el = collectElements();
    this.audio = new AudioEngine();
    this.store = new SaveStore();
    this.settings = loadSettings();
    this.renderer = createRenderer(this.el.canvas, this.settings.shader);
    this.el.canvas = this.renderer.canvas;

    this.emulator = null;
    this.current = null;
    this.keys = 0;
    this.keyboardKeys = 0;
    this.gamepadKeys = 0;
    this.touchKeys = 0;
    this.latchedKeys = 0;
    this.gamepadName = "";
    this.paused = false;
    this.fastForward = false;
    this.audioFailed = false;
    this.hiddenPaused = false;
    this.running = true;
    this.lastTick = performance.now();
    this.accumulator = 0;
    this.needsRender = true;
    this.scale = 1;
    this.fps = 0;
    this.fpsFrames = 0;
    this.fpsWindow = performance.now();
    this.lastHud = 0;
    this.saveStored = false;
    this.saving = false;
    this.toastTimer = 0;
    this.resizeFrame = 0;

    this.applySettings();
    this.bindEvents();
    this.renderBuiltins();
    this.renderRecents();
    this.updateRendererKind();
    this.startLoop();
    this.observeResize();
    this.boot();
  }

  async boot() {
    this.setStatus("Booting…");

    if (this.store) {
      try {
        await this.store.open();
      } catch (error) {
        console.warn("cinnabar: persistence unavailable", error);
        this.store = null;
        this.toast("Browser storage is unavailable; saves will not persist.", true);
      }
    }

    this.updateStorageUi();
    this.renderRecents();

    try {
      await this.loadInitial();
    } catch (error) {
      console.error("cinnabar: boot failed", error);
      this.setStatus("Boot failed");
      this.toast(`Could not boot: ${error.message}`, true);
    }

    this.updateSoundUi();
    this.requestScale();
  }

  async loadInitial() {
    const requested = new URLSearchParams(window.location.search).get("demo");
    if (requested && BUILTINS.some((builtin) => builtin.id === requested)) {
      await this.loadBuiltin(requested, { silent: true });
      return;
    }

    if (this.settings.rememberRom && this.store && this.settings.lastHash) {
      try {
        const record = await this.store.getRom(this.settings.lastHash);
        if (record?.bytes) {
          await this.loadRom(toBytes(record.bytes), record.name ?? "Remembered ROM", {
            remember: true,
            silent: true,
          });
          return;
        }
      } catch (error) {
        console.warn("cinnabar: could not restore the remembered ROM", error);
      }
    }

    await this.loadBuiltin("logo", { silent: true });
  }

  startLoop() {
    const loop = (now) => {
      if (!this.running) {
        return;
      }
      this.tick(now);
      requestAnimationFrame(loop);
    };
    this.lastTick = performance.now();
    this.fpsWindow = this.lastTick;
    requestAnimationFrame(loop);
  }

  tick(now) {
    const dt = Math.min(Math.max(now - this.lastTick, 0), 100);
    this.lastTick = now;

    this.pollGamepad();
    this.syncKeys();

    if (!this.emulator) {
      this.updateHud(now);
      return;
    }

    if (!this.paused) {
      const frames = this.framesToRun(dt);
      for (let index = 0; index < frames; index += 1) {
        this.emulator.run_frame();
        const samples = this.emulator.take_audio();
        if (this.audio.active && !this.fastForward) {
          this.audio.push(samples);
        }
      }

      if (frames > 0) {
        this.latchedKeys = 0;
      }

      if (frames > 0 || this.needsRender) {
        this.renderer.draw(this.emulator.framebuffer_rgba());
        this.needsRender = false;
      }

      this.fpsFrames += frames;
    } else if (this.needsRender) {
      this.renderer.draw(this.emulator.framebuffer_rgba());
      this.needsRender = false;
    }

    if (now - this.fpsWindow >= 500) {
      const elapsed = now - this.fpsWindow;
      this.fps = elapsed > 0 ? (this.fpsFrames * 1000) / elapsed : 0;
      this.fpsFrames = 0;
      this.fpsWindow = now;
      if (!this.paused) {
        this.el.fps.textContent = `${this.fps.toFixed(1)} fps`;
      }
    }

    this.updateHud(now);
  }

  framesToRun(dt) {
    if (this.fastForward) {
      this.accumulator = 0;
      return FAST_FORWARD_FRAMES;
    }

    this.accumulator += dt;
    let frames = 0;
    while (this.accumulator >= FRAME_MS && frames < MAX_CATCHUP_FRAMES) {
      frames += 1;
      this.accumulator -= FRAME_MS;
    }

    // Wait for the first worklet report before steering on the queue. Before
    // that, `queued` still reads zero and we would overfill the buffer by
    // running extra frames during the worklet's startup window.
    if (this.audio.active && this.audio.statsReady) {
      const samplesPerFrame = this.emulator.output_sample_rate() / GBA_FPS;
      const queued = this.audio.queuedFrames;
      if (queued < samplesPerFrame * 0.75) {
        frames = Math.max(frames, 1);
      } else if (queued > samplesPerFrame * 5) {
        frames = 0;
      }
    }

    if (this.accumulator > FRAME_MS * MAX_CATCHUP_FRAMES) {
      this.accumulator = FRAME_MS;
    }

    return frames;
  }

  async loadBuiltin(id, options = {}) {
    const builtin = BUILTINS.find((entry) => entry.id === id);
    if (!builtin) {
      return;
    }

    await this.flushSave();

    try {
      const emulator = Emulator.builtin(id);
      this.adopt(emulator, {
        name: builtin.name,
        title: emulator.title(),
        hash: null,
        builtin: id,
      });
      this.saveStored = false;
      this.updateSaveUi();
      if (!options.silent) {
        this.toast(`Loaded ${builtin.name}`);
      }
    } catch (error) {
      console.error("cinnabar: built-in failed to load", error);
      this.toast(`Could not load ${builtin.name}: ${error.message}`, true);
    }
  }

  async loadRom(bytes, name, options = {}) {
    const remember = options.remember ?? this.settings.rememberRom;
    const data = toBytes(bytes);
    if (data.length < 0xc0) {
      throw new Error("That file is too small to be a GBA ROM.");
    }
    if (data.length > 64 * 1024 * 1024) {
      throw new Error("That ROM is larger than 64 MB.");
    }

    const hash = await hashRom(data);
    await this.flushSave();

    const emulator = new Emulator(data);
    let storedSave = null;
    if (this.store) {
      try {
        storedSave = await this.store.getSave(hash);
      } catch (error) {
        console.warn("cinnabar: could not read the stored save", error);
      }
    }

    if (storedSave?.bytes) {
      const savedBytes = toBytes(storedSave.bytes);
      if (savedBytes.length > 0) {
        emulator.load_save(savedBytes);
        emulator.mark_save_clean();
      }
    }

    this.adopt(emulator, {
      name,
      title: emulator.title(),
      hash,
      builtin: null,
      bytes: data,
    });
    this.saveStored = Boolean(storedSave?.bytes?.length);

    if (remember && this.store) {
      try {
        await this.store.putRom({
          hash,
          name,
          title: emulator.title(),
          bytes: data,
          size: data.length,
        });
        this.settings.lastHash = hash;
        saveSettings(this.settings);
        this.renderRecents();
      } catch (error) {
        console.warn("cinnabar: could not remember the ROM", error);
        this.settings.lastHash = null;
        saveSettings(this.settings);
        this.toast("Could not remember this ROM.", true);
      }
    } else {
      this.settings.lastHash = null;
      saveSettings(this.settings);
    }

    this.updateSaveUi();
    if (!options.silent) {
      this.toast(`Loaded ${emulator.title() || name}`);
    }
  }

  adopt(emulator, meta) {
    try {
      this.emulator?.free();
    } catch {
      // The old instance may already have been released.
    }

    this.emulator = emulator;
    this.current = meta;
    this.keyboardKeys = 0;
    this.gamepadKeys = 0;
    this.touchKeys = 0;
    this.latchedKeys = 0;
    this.keys = 0;
    this.emulator.set_keys(0);
    this.emulator.set_output_rate(this.audio.active ? this.audio.sampleRate : 48000);
    this.audio.reset();
    this.accumulator = 0;
    this.lastTick = performance.now();
    this.needsRender = true;
    this.saveStored = false;
    this.saving = false;
    this.setPaused(false);
    this.updateNowPlaying();
    this.updateSaveUi();
    this.renderBuiltins();
    this.updateRendererKind();
    this.lastHud = 0;
    this.el.cabinet.focus({ preventScroll: true });
  }

  updateNowPlaying() {
    if (!this.current) {
      return;
    }
    const title = this.current.builtin
      ? this.current.name
      : this.current.title && this.current.title !== "Untitled cartridge"
        ? this.current.title
        : this.current.name;
    this.el.nowPlaying.textContent = title;
    this.el.nowPlaying.title = title;
    document.title = `${title} — Cinnabar Arcade`;
    this.el.status.textContent = "Running";
  }

  setPaused(paused) {
    this.paused = paused;
    this.el.pause.classList.toggle("active", paused);
    this.el.pause.innerHTML = paused ? "Resume <kbd>F1</kbd>" : "Pause <kbd>F1</kbd>";
    this.el.pauseBadge.hidden = !paused;
    this.audio.setPaused(paused);
    if (paused) {
      this.latchedKeys = 0;
      this.el.fps.textContent = "paused";
      this.el.status.textContent = "Paused";
      this.flushSave();
    } else if (this.emulator) {
      this.el.status.textContent = "Running";
      this.el.fps.textContent = `${this.fps.toFixed(1)} fps`;
    }
    this.lastTick = performance.now();
    this.accumulator = 0;
  }

  toggleFastForward() {
    this.fastForward = !this.fastForward;
    this.el.fastForward.classList.toggle("active", this.fastForward);
    if (this.fastForward) {
      this.toast("Fast-forward on");
    }
  }

  resetEmulator() {
    if (!this.emulator) {
      return;
    }
    this.emulator.reset();
    this.audio.reset();
    this.accumulator = 0;
    this.needsRender = true;
    this.updateSaveUi();
    this.toast("Console reset");
  }

  async flushSave(force = false) {
    if (!this.emulator || !this.current || this.current.builtin) {
      return;
    }
    if (!this.store || !this.current.hash) {
      return;
    }
    if (!force && !this.emulator.save_dirty()) {
      return;
    }
    if (this.saving) {
      return;
    }

    const bytes = this.emulator.save_data();
    if (bytes.length === 0) {
      return;
    }

    this.saving = true;
    this.updateSaveUi();
    try {
      await this.store.putSave(this.current.hash, bytes);
      this.emulator.mark_save_clean();
      this.saveStored = true;
    } catch (error) {
      console.warn("cinnabar: save failed", error);
      this.toast("Could not write the save to this browser.", true);
    } finally {
      this.saving = false;
      this.updateSaveUi();
    }
  }

  updateSaveUi() {
    if (!this.emulator) {
      return;
    }

    const saveType = this.emulator.save_type();
    const bytes = this.emulator.save_data();
    const saveable = saveType !== "none" && bytes.length > 0;

    this.el.saveType.textContent = saveable ? saveType : "No battery save";
    this.el.exportSave.disabled = !saveable;
    this.el.importSave.disabled = !saveable;
    this.el.forgetSave.disabled =
      !saveable || !this.store || !this.current || Boolean(this.current.builtin);
    this.el.rememberRom.checked = this.settings.rememberRom;
    this.el.rememberRom.disabled = !this.store;

    if (!saveable) {
      this.el.saveStatus.textContent = "This cartridge does not use battery-backed saves.";
    } else if (this.saving) {
      this.el.saveStatus.textContent = "Saving…";
    } else if (this.emulator.save_dirty()) {
      this.el.saveStatus.textContent = "Changes waiting to be written";
    } else if (this.saveStored) {
      this.el.saveStatus.textContent = "Saved to this browser";
    } else {
      this.el.saveStatus.textContent = "Ready — no saved data yet";
    }
  }

  exportSave() {
    const bytes = this.emulator?.save_data();
    if (!bytes || bytes.length === 0) {
      this.toast("This cartridge has no battery save.", true);
      return;
    }
    const title = sanitizeFilename(this.current?.title || this.current?.name || "cinnabar");
    download(`${title}.sav`, bytes);
    this.toast("Save exported");
  }

  async importSave(file) {
    if (!file || !this.emulator) {
      return;
    }
    if (this.emulator.save_type() === "none") {
      this.toast("This cartridge has no battery save to import into.", true);
      return;
    }

    try {
      const bytes = new Uint8Array(await file.arrayBuffer());
      this.emulator.load_save(bytes);
      const normalized = this.emulator.save_data();
      if (this.store && this.current?.hash) {
        await this.store.putSave(this.current.hash, normalized);
        this.saveStored = true;
        this.emulator.mark_save_clean();
      }
      this.updateSaveUi();
      this.toast("Save imported");
    } catch (error) {
      console.error("cinnabar: import failed", error);
      this.toast(`Could not import the save: ${error.message}`, true);
    }
  }

  async forgetSave() {
    if (!this.emulator || !this.current || this.current.builtin) {
      return;
    }
    if (!window.confirm("Forget this game's saved data in this browser?")) {
      return;
    }

    try {
      const size = this.emulator.save_data().length;
      if (size > 0) {
        this.emulator.load_save(new Uint8Array(size).fill(0xff));
        this.emulator.mark_save_clean();
      }
      if (this.store && this.current.hash) {
        await this.store.deleteSave(this.current.hash);
      }
      this.saveStored = false;
      this.updateSaveUi();
      this.toast("Saved data forgotten");
    } catch (error) {
      console.error("cinnabar: could not forget save", error);
      this.toast("Could not forget the saved data.", true);
    }
  }

  async loadFile(file) {
    if (!file) {
      return;
    }
    if (file.size > 64 * 1024 * 1024) {
      this.toast("That ROM is larger than 64 MB.", true);
      return;
    }

    this.setStatus("Reading ROM…");
    try {
      const bytes = new Uint8Array(await file.arrayBuffer());
      await this.loadRom(bytes, file.name, { remember: this.settings.rememberRom });
    } catch (error) {
      console.error("cinnabar: ROM load failed", error);
      this.setStatus("Ready");
      this.toast(error.message, true);
    }
  }

  async enableAudio() {
    if (this.audio.active || this.audio.starting) {
      return;
    }

    this.audioFailed = false;
    try {
      const rate = await this.audio.start();
      this.emulator?.set_output_rate(rate);
      this.applyVolumeSetting();
      this.updateSoundUi();
      this.toast("Sound on");
    } catch (error) {
      console.warn("cinnabar: audio unavailable", error);
      this.audioFailed = true;
      this.updateSoundUi();
      this.toast(`Audio unavailable: ${error.message}`, true);
    }
  }

  applyVolumeSetting() {
    this.audio.setVolume(this.settings.muted ? 0 : this.settings.volume);
    this.updateMuteUi();
  }

  toggleMute() {
    this.settings.muted = !this.settings.muted;
    this.applyVolumeSetting();
    this.updateSoundUi();
    saveSettings(this.settings);
  }

  updateMuteUi() {
    const muted = this.settings.muted || this.settings.volume === 0;
    this.el.mute.classList.toggle("muted", muted);
    this.el.mute.setAttribute("aria-label", muted ? "Unmute sound" : "Mute sound");
  }

  updateSoundUi() {
    if (this.audio.active) {
      this.el.soundPill.hidden = true;
      return;
    }
    this.el.soundPill.hidden = false;
    this.el.soundPill.dataset.state = this.audioFailed ? "unavailable" : "off";
    this.el.soundLabel.textContent = this.audioFailed
      ? "Audio unavailable in this browser"
      : "Sound off · tap to enable";
  }

  updateShader() {
    this.renderer.setPreset(this.settings.shader);
    this.el.bezel.dataset.effects =
      this.renderer.kind === "2d" ? this.settings.shader : "webgl";
    this.el.shader.value = this.settings.shader;
  }

  updateRendererKind() {
    this.el.rendererKind.textContent =
      this.renderer.kind === "webgl2" ? "WebGL2" : "Canvas 2D";
  }

  applySettings() {
    this.el.volume.value = String(this.settings.volume);
    this.el.shader.value = this.settings.shader;
    this.el.rememberRom.checked = this.settings.rememberRom;
    this.el.hud.hidden = !this.settings.hud;
    this.applyVolumeSetting();
    this.updateShader();
    this.updateSoundUi();
  }

  updateStorageUi() {
    const available = Boolean(this.store);
    this.el.clearRoms.disabled = !available;
    this.el.rememberRom.disabled = !available;
    if (!available) {
      this.el.recentList.innerHTML =
        '<p class="recent-empty">Browser storage is unavailable.</p>';
    }
  }

  async renderRecents() {
    if (!this.store) {
      return;
    }

    let records = [];
    try {
      records = await this.store.listRoms();
    } catch (error) {
      console.warn("cinnabar: could not list remembered ROMs", error);
      return;
    }

    if (records.length === 0) {
      const empty = document.createElement("p");
      empty.className = "recent-empty";
      empty.textContent = "No remembered ROMs yet.";
      this.el.recentList.replaceChildren(empty);
      return;
    }

    const fragment = document.createDocumentFragment();
    for (const record of records.slice(0, 8)) {
      const row = document.createElement("div");
      row.className = "recent-item";

      const load = document.createElement("button");
      load.type = "button";
      load.className = "recent-load";
      load.textContent = record.name || record.title || record.hash.slice(0, 10);
      load.title = `${record.title || "ROM"} · ${formatBytes(record.size)}`;
      if (this.current?.hash === record.hash) {
        load.classList.add("active");
      }
      load.addEventListener("click", async () => {
        try {
          const full = await this.store.getRom(record.hash);
          if (!full?.bytes) {
            this.toast("That remembered ROM is no longer available.", true);
            return;
          }
          await this.loadRom(toBytes(full.bytes), record.name, { remember: true });
        } catch (error) {
          this.toast(error.message, true);
        }
      });

      const remove = document.createElement("button");
      remove.type = "button";
      remove.className = "recent-remove";
      remove.textContent = "×";
      remove.title = "Remove from this device";
      remove.setAttribute("aria-label", `Remove ${load.textContent} from this device`);
      remove.addEventListener("click", async () => {
        try {
          await this.store.deleteRom(record.hash);
          if (this.settings.lastHash === record.hash) {
            this.settings.lastHash = null;
            saveSettings(this.settings);
          }
          this.renderRecents();
          this.toast("Remembered ROM removed");
        } catch (error) {
          console.error("cinnabar: could not remove ROM", error);
        }
      });

      row.append(load, remove);
      fragment.append(row);
    }
    this.el.recentList.replaceChildren(fragment);
  }

  async clearRememberedRoms() {
    if (!this.store) {
      return;
    }
    if (
      !window.confirm(
        "Forget all remembered ROMs in this browser? Battery saves will be kept.",
      )
    ) {
      return;
    }
    try {
      await this.store.clearRoms();
      this.settings.lastHash = null;
      saveSettings(this.settings);
      this.renderRecents();
      this.toast("Remembered ROMs cleared");
    } catch (error) {
      console.error("cinnabar: could not clear remembered ROMs", error);
      this.toast("Could not clear remembered ROMs.", true);
    }
  }

  renderBuiltins() {
    const fragment = document.createDocumentFragment();
    for (const builtin of BUILTINS) {
      const button = document.createElement("button");
      button.type = "button";
      button.className = "cart-chip";
      if (this.current?.builtin === builtin.id) {
        button.classList.add("active");
      }
      button.innerHTML = `
        <span class="cart-icon" aria-hidden="true">${builtin.code}</span>
        <span>
          <strong>${builtin.name}</strong>
          <small>${builtin.note}</small>
        </span>
      `;
      button.addEventListener("click", () => this.loadBuiltin(builtin.id));
      fragment.append(button);
    }
    this.el.builtinList.replaceChildren(fragment);
  }

  syncKeys() {
    const keys = this.keyboardKeys | this.gamepadKeys | this.touchKeys | this.latchedKeys;
    if (keys === this.keys) {
      return;
    }
    this.keys = keys;
    this.emulator?.set_keys(keys);
  }

  pollGamepad() {
    let bits = 0;
    let name = "";

    try {
      const pads = navigator.getGamepads?.() ?? [];
      for (const pad of pads) {
        if (!pad) {
          continue;
        }
        name = name || pad.id;
        const pressed = (index) => {
          const button = pad.buttons[index];
          return Boolean(button?.pressed) || (button?.value ?? 0) > 0.5;
        };

        if (pressed(0)) bits |= KEYS.A;
        if (pressed(1)) bits |= KEYS.B;
        if (pressed(8)) bits |= KEYS.SELECT;
        if (pressed(9)) bits |= KEYS.START;
        if (pressed(4)) bits |= KEYS.L;
        if (pressed(5)) bits |= KEYS.R;
        if (pressed(12)) bits |= KEYS.UP;
        if (pressed(13)) bits |= KEYS.DOWN;
        if (pressed(14)) bits |= KEYS.LEFT;
        if (pressed(15)) bits |= KEYS.RIGHT;

        const axisX = pad.axes[0] ?? 0;
        const axisY = pad.axes[1] ?? 0;
        const hatX = pad.axes[6] ?? 0;
        const hatY = pad.axes[7] ?? 0;
        if (axisX < -0.45 || hatX < -0.5) bits |= KEYS.LEFT;
        if (axisX > 0.45 || hatX > 0.5) bits |= KEYS.RIGHT;
        if (axisY < -0.45 || hatY < -0.5) bits |= KEYS.UP;
        if (axisY > 0.45 || hatY > 0.5) bits |= KEYS.DOWN;
      }
    } catch (error) {
      console.warn("cinnabar: gamepad polling failed", error);
    }

    // Latch newly seen button presses for at least one emulated frame. A very
    // quick press can otherwise begin and end between two polls.
    this.latchedKeys |= bits & ~this.gamepadKeys;
    this.gamepadKeys = bits;
    this.gamepadName = name;
  }

  setTouchKey(bit, pressed) {
    if (pressed) {
      this.touchKeys |= bit;
    } else {
      this.touchKeys &= ~bit;
    }
    this.syncKeys();
  }

  bindTouchControls() {
    for (const button of this.el.touchControls.querySelectorAll("[data-touch]")) {
      const bit = KEYS[button.dataset.touch];
      if (bit === undefined) {
        continue;
      }
      const pointers = new Set();

      const press = (event) => {
        event.preventDefault();
        pointers.add(event.pointerId);
        button.setPointerCapture?.(event.pointerId);
        button.classList.add("pressed");
        this.latchedKeys |= bit;
        this.setTouchKey(bit, true);
        this.enableAudio();
      };

      const release = (event) => {
        if (!pointers.delete(event.pointerId)) {
          return;
        }
        if (pointers.size === 0) {
          button.classList.remove("pressed");
          this.setTouchKey(bit, false);
        }
      };

      button.addEventListener("pointerdown", press);
      button.addEventListener("pointerup", release);
      button.addEventListener("pointercancel", release);
      button.addEventListener("lostpointercapture", release);
      button.addEventListener("contextmenu", (event) => event.preventDefault());
    }
  }

  bindEvents() {
    this.el.pause.addEventListener("click", () => this.setPaused(!this.paused));
    this.el.fastForward.addEventListener("click", () => this.toggleFastForward());
    this.el.reset.addEventListener("click", () => this.resetEmulator());
    this.el.screenshot.addEventListener("click", () => this.takeScreenshot());
    this.el.fullscreen.addEventListener("click", () => this.toggleFullscreen());
    this.el.hudToggle.addEventListener("click", () => this.toggleHud());
    this.el.mute.addEventListener("click", () => this.toggleMute());
    this.el.soundPill.addEventListener("click", () => {
      this.enableAudio();
      this.el.cabinet.focus({ preventScroll: true });
    });
    this.el.bezel.addEventListener("pointerdown", () => {
      this.el.cabinet.focus({ preventScroll: true });
    });

    this.el.volume.addEventListener("input", () => {
      this.settings.volume = Number(this.el.volume.value);
      if (this.settings.volume > 0) {
        this.settings.muted = false;
      }
      this.applyVolumeSetting();
      this.updateSoundUi();
      saveSettings(this.settings);
    });

    this.el.shader.addEventListener("change", () => {
      this.settings.shader = this.el.shader.value;
      this.updateShader();
      saveSettings(this.settings);
    });

    this.el.rememberRom.addEventListener("change", async () => {
      this.settings.rememberRom = this.el.rememberRom.checked;
      saveSettings(this.settings);
      if (this.settings.rememberRom && this.current?.bytes && this.store) {
        try {
          await this.store.putRom({
            hash: this.current.hash,
            name: this.current.name,
            title: this.current.title,
            bytes: this.current.bytes,
            size: this.current.bytes.length,
          });
          this.settings.lastHash = this.current.hash;
          saveSettings(this.settings);
          this.renderRecents();
          this.toast("ROM remembered on this device");
        } catch (error) {
          console.warn("cinnabar: could not remember ROM", error);
          this.toast("Could not remember this ROM.", true);
        }
      } else if (!this.settings.rememberRom) {
        this.settings.lastHash = null;
        saveSettings(this.settings);
      }
    });

    this.el.romInput.addEventListener("change", async () => {
      const file = this.el.romInput.files?.[0];
      this.el.romInput.value = "";
      await this.loadFile(file);
    });

    this.el.importSaveInput.addEventListener("change", async () => {
      const file = this.el.importSaveInput.files?.[0];
      this.el.importSaveInput.value = "";
      await this.importSave(file);
    });
    this.el.importSave.addEventListener("click", () => this.el.importSaveInput.click());
    this.el.exportSave.addEventListener("click", () => this.exportSave());
    this.el.forgetSave.addEventListener("click", () => this.forgetSave());
    this.el.clearRoms.addEventListener("click", () => this.clearRememberedRoms());

    const openHelp = () => {
      if (!this.el.helpDialog.open) {
        this.el.helpDialog.showModal();
      }
    };
    this.el.helpButton.addEventListener("click", openHelp);
    this.el.footerHelp.addEventListener("click", openHelp);
    this.el.helpClose.addEventListener("click", () => this.el.helpDialog.close());
    this.el.helpDialog.addEventListener("click", (event) => {
      if (event.target === this.el.helpDialog) {
        this.el.helpDialog.close();
      }
    });

    this.el.dropzone.addEventListener("dragover", (event) => {
      event.preventDefault();
      this.el.dropzone.classList.add("dragging");
    });
    this.el.dropzone.addEventListener("dragleave", () => {
      this.el.dropzone.classList.remove("dragging");
    });
    this.el.dropzone.addEventListener("drop", async (event) => {
      event.preventDefault();
      this.el.dropzone.classList.remove("dragging");
      await this.loadFile(event.dataTransfer?.files?.[0]);
    });

    window.addEventListener("dragover", (event) => event.preventDefault());
    window.addEventListener("drop", (event) => event.preventDefault());

    window.addEventListener("keydown", (event) => this.handleKeydown(event));
    window.addEventListener("keyup", (event) => this.handleKeyup(event));
    window.addEventListener("blur", () => {
      this.keyboardKeys = 0;
      this.touchKeys = 0;
      this.gamepadKeys = 0;
      this.latchedKeys = 0;
      this.syncKeys();
    });

    window.addEventListener("gamepadconnected", (event) => {
      this.toast(`Gamepad connected: ${event.gamepad.id}`);
    });
    window.addEventListener("gamepaddisconnected", () => {
      this.toast("Gamepad disconnected");
    });

    document.addEventListener("fullscreenchange", () => {
      const active = document.fullscreenElement === this.el.cabinet;
      this.el.fullscreen.textContent = active ? "Exit fullscreen" : "Fullscreen";
      this.requestScale();
    });

    document.addEventListener("visibilitychange", () => {
      if (document.hidden) {
        this.flushSave();
        this.hiddenPaused = !this.paused;
        if (this.hiddenPaused) {
          this.setPaused(true);
        }
      } else if (this.hiddenPaused) {
        this.hiddenPaused = false;
        this.setPaused(false);
        this.lastTick = performance.now();
        this.fpsWindow = this.lastTick;
        this.fpsFrames = 0;
      }
    });

    window.addEventListener("pagehide", () => this.flushSave(true));

    const unlockAudio = (event) => {
      if (event.type === "keydown" && isTextTarget(event.target)) {
        return;
      }
      this.enableAudio();
      window.removeEventListener("pointerdown", unlockAudio, true);
      window.removeEventListener("keydown", unlockAudio, true);
    };
    window.addEventListener("pointerdown", unlockAudio, true);
    window.addEventListener("keydown", unlockAudio, true);

    this.bindTouchControls();

    setInterval(() => this.flushSave(), 5000);
  }

  handleKeydown(event) {
    if (isTextTarget(event.target)) {
      return;
    }
    if (
      event.target instanceof HTMLElement &&
      (event.target.tagName === "BUTTON" || event.target.tagName === "A") &&
      (event.code === "Enter" || event.code === "Space")
    ) {
      return;
    }

    switch (event.code) {
      case "F1":
      case "KeyP":
        event.preventDefault();
        if (!event.repeat) {
          this.setPaused(!this.paused);
        }
        return;
      case "F2":
        event.preventDefault();
        if (!event.repeat) {
          this.toggleFastForward();
        }
        return;
      case "F3":
        event.preventDefault();
        if (!event.repeat) {
          this.toggleHud();
        }
        return;
      case "F5":
        event.preventDefault();
        if (!event.repeat) {
          this.takeScreenshot();
        }
        return;
      case "F11":
        event.preventDefault();
        if (!event.repeat) {
          this.toggleFullscreen();
        }
        return;
      case "KeyM":
        event.preventDefault();
        if (!event.repeat) {
          this.toggleMute();
        }
        return;
      default:
        break;
    }

    const bit = KEY_MAP[event.code];
    if (bit !== undefined) {
      event.preventDefault();
      this.keyboardKeys |= bit;
      if (!event.repeat) {
        this.latchedKeys |= bit;
      }
      this.syncKeys();
      if (!this.audio.active) {
        this.enableAudio();
      }
    }
  }

  handleKeyup(event) {
    if (isTextTarget(event.target)) {
      return;
    }
    const bit = KEY_MAP[event.code];
    if (bit !== undefined) {
      event.preventDefault();
      this.keyboardKeys &= ~bit;
      this.syncKeys();
    }
  }

  toggleHud() {
    this.settings.hud = !this.settings.hud;
    this.el.hud.hidden = !this.settings.hud;
    saveSettings(this.settings);
    this.lastHud = 0;
    this.updateHud(performance.now());
  }

  updateHud(now) {
    if (!this.settings.hud || !this.emulator) {
      return;
    }
    if (now - this.lastHud < 250) {
      return;
    }
    this.lastHud = now;

    this.el.hudFps.textContent = `${this.fps.toFixed(1)} fps`;
    this.el.hudAudio.textContent = this.audio.active
      ? `${Math.round((this.audio.queuedFrames / this.audio.sampleRate) * 1000)} ms`
      : "off";
    this.el.hudUnderruns.textContent = this.audio.active
      ? `${this.audio.stats.underruns} (${this.audio.stats.overruns} over)`
      : "—";
    this.el.hudRenderer.textContent =
      this.renderer.kind === "webgl2" ? "WebGL2" : "Canvas 2D";
    this.el.hudGamepad.textContent = this.gamepadName || "none";
    this.el.hudScale.textContent = `${this.scale}×`;
  }

  takeScreenshot() {
    if (!this.emulator) {
      return;
    }
    const canvas = document.createElement("canvas");
    canvas.width = 240;
    canvas.height = 160;
    const context = canvas.getContext("2d");
    if (!context) {
      return;
    }
    const image = context.createImageData(240, 160);
    image.data.set(this.emulator.framebuffer_rgba());
    context.putImageData(image, 0, 0);
    canvas.toBlob((blob) => {
      if (!blob) {
        return;
      }
      const url = URL.createObjectURL(blob);
      const link = document.createElement("a");
      link.href = url;
      link.download = `${sanitizeFilename(this.current?.title || "cinnabar")}-${Date.now()}.png`;
      link.click();
      setTimeout(() => URL.revokeObjectURL(url), 1000);
      this.toast("Screenshot saved");
    }, "image/png");
  }

  async toggleFullscreen() {
    try {
      if (document.fullscreenElement) {
        await document.exitFullscreen();
      } else {
        await this.el.cabinet.requestFullscreen();
        try {
          await window.screen.orientation?.lock?.("landscape");
        } catch {
          // Orientation lock is best-effort and desktop browsers reject it.
        }
      }
    } catch (error) {
      console.warn("cinnabar: fullscreen failed", error);
      this.toast("Fullscreen was blocked by the browser.", true);
    }
    this.requestScale();
  }

  observeResize() {
    const pointerQuery = window.matchMedia("(any-pointer: coarse)");
    this.coarsePointer = pointerQuery.matches;
    const onPointerChange = (event) => {
      this.coarsePointer = event.matches;
      this.requestScale();
    };
    if (pointerQuery.addEventListener) {
      pointerQuery.addEventListener("change", onPointerChange);
    } else {
      pointerQuery.addListener(onPointerChange);
    }
    if ("ResizeObserver" in window) {
      this.resizeObserver = new ResizeObserver(() => this.requestScale());
      this.resizeObserver.observe(this.el.bezel);
    }
    window.addEventListener("resize", () => this.requestScale());
    window.visualViewport?.addEventListener("resize", () => this.requestScale());
  }

  requestScale() {
    cancelAnimationFrame(this.resizeFrame);
    this.resizeFrame = requestAnimationFrame(() => this.updateScale());
  }

  updateScale() {
    const width = this.el.bezel.clientWidth - 2;
    const height = this.el.bezel.clientHeight - 2;
    if (width <= 0 || height <= 0) {
      return;
    }
    const fit = Math.max(1, Math.min(width / 240, height / 160));
    // Desktop keeps crisp integer scales. Touch devices fill the bezel so the
    // screen is not tiny in portrait; nearest-neighbour keeps pixels sharp.
    const scale = this.coarsePointer
      ? Math.max(1, Math.floor(fit * 100) / 100)
      : Math.max(1, Math.floor(fit));
    if (scale !== this.scale) {
      this.scale = scale;
      this.renderer.resize(scale);
    }
  }

  setStatus(text) {
    this.el.status.textContent = text;
  }

  toast(message, error = false) {
    clearTimeout(this.toastTimer);
    this.el.toast.textContent = message;
    this.el.toast.classList.toggle("error", error);
    this.el.toast.classList.add("visible");
    this.toastTimer = setTimeout(() => {
      this.el.toast.classList.remove("visible");
    }, 3200);
  }
}

const arcade = new Arcade();
window.cinnabar = arcade;
globalThis.__cinnabarArcade = arcade;
document.documentElement.dataset.arcadeReady = "yes";

function collectElements() {
  return {
    cabinet: byId("cabinet"),
    bezel: byId("bezel"),
    canvas: byId("screen"),
    nowPlaying: byId("now-playing"),
    fps: byId("fps"),
    status: byId("status"),
    rendererKind: byId("renderer-kind"),
    hud: byId("hud"),
    hudFps: byId("hud-fps"),
    hudAudio: byId("hud-audio"),
    hudUnderruns: byId("hud-underruns"),
    hudRenderer: byId("hud-renderer"),
    hudGamepad: byId("hud-gamepad"),
    hudScale: byId("hud-scale"),
    pauseBadge: byId("pause-badge"),
    soundPill: byId("sound-pill"),
    soundLabel: byId("sound-label"),
    pause: byId("pause"),
    fastForward: byId("fast-forward"),
    reset: byId("reset"),
    screenshot: byId("screenshot"),
    fullscreen: byId("fullscreen"),
    hudToggle: byId("hud-toggle"),
    mute: byId("mute"),
    volume: byId("volume"),
    shader: byId("shader"),
    builtinList: byId("builtin-list"),
    dropzone: byId("dropzone"),
    romInput: byId("rom-input"),
    recentList: byId("recent-list"),
    clearRoms: byId("clear-roms"),
    saveType: byId("save-type"),
    saveStatus: byId("save-status"),
    exportSave: byId("export-save"),
    importSave: byId("import-save"),
    importSaveInput: byId("import-save-input"),
    forgetSave: byId("forget-save"),
    rememberRom: byId("remember-rom"),
    helpButton: byId("help-button"),
    helpDialog: byId("help-dialog"),
    helpClose: byId("help-close"),
    footerHelp: byId("footer-help"),
    touchControls: byId("touch-controls"),
    toast: byId("toast"),
  };
}

function byId(id) {
  const element = document.getElementById(id);
  if (!element) {
    throw new Error(`missing element #${id}`);
  }
  return element;
}

async function bootWasm() {
  try {
    await init();
  } catch (error) {
    console.error("cinnabar: WebAssembly failed to load", error);
    const status = document.getElementById("status");
    if (status) {
      status.textContent = "WebAssembly failed to load";
    }
    throw error;
  }
}

function loadSettings() {
  try {
    const raw = localStorage.getItem(SETTINGS_KEY);
    if (!raw) {
      return { ...DEFAULT_SETTINGS };
    }
    const parsed = JSON.parse(raw);
    return {
      ...DEFAULT_SETTINGS,
      ...parsed,
      volume: clamp(Number(parsed.volume ?? DEFAULT_SETTINGS.volume), 0, 1),
      muted: Boolean(parsed.muted),
      shader: ["off", "lcd", "crt"].includes(parsed.shader)
        ? parsed.shader
        : DEFAULT_SETTINGS.shader,
      rememberRom: Boolean(parsed.rememberRom),
      hud: Boolean(parsed.hud),
      lastHash: typeof parsed.lastHash === "string" ? parsed.lastHash : null,
    };
  } catch {
    return { ...DEFAULT_SETTINGS };
  }
}

function saveSettings(settings) {
  try {
    localStorage.setItem(SETTINGS_KEY, JSON.stringify(settings));
  } catch {
    // Private mode and storage pressure are not fatal.
  }
}

function toBytes(value) {
  if (value instanceof Uint8Array) {
    return value;
  }
  if (value instanceof ArrayBuffer) {
    return new Uint8Array(value);
  }
  return new Uint8Array(value);
}

function isTextTarget(target) {
  return (
    target instanceof HTMLElement &&
    (target.isContentEditable || ["INPUT", "SELECT", "TEXTAREA"].includes(target.tagName))
  );
}

function formatBytes(bytes) {
  if (!Number.isFinite(bytes)) {
    return "unknown size";
  }
  if (bytes >= 1024 * 1024) {
    return `${(bytes / (1024 * 1024)).toFixed(1)} MiB`;
  }
  if (bytes >= 1024) {
    return `${Math.round(bytes / 1024)} KiB`;
  }
  return `${bytes} B`;
}

function sanitizeFilename(value) {
  return value
    .replace(/[^a-z0-9._-]+/gi, "-")
    .replace(/^-+|-+$/g, "")
    .slice(0, 80) || "cinnabar";
}

function download(filename, bytes) {
  const blob = new Blob([bytes], { type: "application/octet-stream" });
  const url = URL.createObjectURL(blob);
  const link = document.createElement("a");
  link.href = url;
  link.download = filename;
  link.click();
  setTimeout(() => URL.revokeObjectURL(url), 1000);
}

function clamp(value, min, max) {
  return Math.min(max, Math.max(min, value));
}
