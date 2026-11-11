// Web Audio output for the browser arcade.
//
// The emulator produces 48 kHz stereo samples (or a resampled rate matching
// the AudioContext). Those samples are transferred to an AudioWorklet, which
// owns the queue and drains it on the audio thread. Keeping the queue off the
// main thread means a busy frame cannot stutter the sound.

export class AudioEngine {
  constructor() {
    this.ctx = null;
    this.node = null;
    this.gain = null;
    this.active = false;
    this.sampleRate = 48000;
    this.volume = 0.75;
    this.paused = false;
    this.stats = { type: "stats", queued: 0, underruns: 0, overruns: 0 };
    this.statsReady = false;
    this.starting = null;
  }

  async start() {
    if (this.active && this.ctx) {
      await this.ctx.resume();
      return this.sampleRate;
    }
    if (this.starting) {
      return this.starting;
    }

    this.starting = this.startInternal().finally(() => {
      this.starting = null;
    });
    return this.starting;
  }

  async startInternal() {
    const Context = window.AudioContext ?? window.webkitAudioContext;
    if (!Context) {
      throw new Error("Web Audio is not supported in this browser");
    }

    let ctx;
    try {
      ctx = new Context({ latencyHint: "interactive", sampleRate: 48000 });
    } catch {
      ctx = new Context({ latencyHint: "interactive" });
    }

    // Resume synchronously inside the user gesture. Safari is strict about
    // this, and awaiting the worklet module can outlive the activation.
    const resume = ctx.resume().catch(() => {});

    try {
      if (!ctx.audioWorklet) {
        throw new Error("AudioWorklet is not supported in this browser");
      }

      await ctx.audioWorklet.addModule(new URL("./audio-worklet.js", import.meta.url));

      const node = new AudioWorkletNode(ctx, "cinnabar-audio", {
        numberOfInputs: 0,
        numberOfOutputs: 1,
        outputChannelCount: [2],
      });
      const gain = ctx.createGain();
      gain.gain.value = 0;
      node.port.onmessage = (event) => {
        if (event.data?.type === "stats") {
          this.stats = event.data;
          this.statsReady = true;
        }
      };
      node.connect(gain).connect(ctx.destination);

      await ctx.resume();
      if (ctx.state !== "running") {
        throw new Error("the browser kept the audio context suspended");
      }

      this.ctx = ctx;
      this.node = node;
      this.gain = gain;
      this.sampleRate = ctx.sampleRate;
      this.active = true;
      this.applyVolume(0.04);
      return this.sampleRate;
    } catch (error) {
      await ctx.close().catch(() => {});
      throw error;
    }
  }

  push(samples) {
    if (!this.active || !this.node || samples.length < 2) {
      return;
    }
    this.node.port.postMessage({ type: "samples", data: samples }, [samples.buffer]);
  }

  reset() {
    if (this.node) {
      this.node.port.postMessage({ type: "reset" });
    }
    this.stats = { type: "stats", queued: 0, underruns: 0, overruns: 0 };
    this.statsReady = false;
  }

  setVolume(volume) {
    this.volume = Math.min(1, Math.max(0, volume));
    this.applyVolume();
  }

  setPaused(paused) {
    this.paused = paused;
    this.applyVolume();
  }

  applyVolume(timeConstant = 0.02) {
    if (!this.gain || !this.ctx) {
      return;
    }
    const value = this.paused ? 0 : this.volume;
    const now = this.ctx.currentTime;
    this.gain.gain.cancelScheduledValues(now);
    this.gain.gain.setTargetAtTime(value, now, timeConstant);
  }

  get queuedFrames() {
    return this.active ? this.stats.queued : 0;
  }
}
