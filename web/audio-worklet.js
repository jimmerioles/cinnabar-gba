// Cinnabar's audio queue lives on the audio thread.
//
// The main thread transfers one Float32Array of interleaved stereo samples per
// emulated frame. This processor drains those chunks at the device clock,
// smooths underruns with a short gain ramp, and reports queue health back to
// the page so the frame loop can stay at a comfortable latency.

class CinnabarAudioProcessor extends AudioWorkletProcessor {
  constructor() {
    super();
    this.chunks = [];
    this.offset = 0;
    this.queued = 0;
    this.underruns = 0;
    this.overruns = 0;
    this.fade = 0;
    this.target = 0;
    this.wasEmpty = true;
    this.statCounter = 0;
    this.maxQueued = Math.max(1, Math.round(sampleRate * 1.5));
    this.port.onmessage = (event) => this.onMessage(event.data);
  }

  onMessage(message) {
    if (!message) {
      return;
    }

    if (message.type === "reset") {
      this.reset();
      return;
    }

    if (message.type !== "samples" || !(message.data instanceof Float32Array)) {
      return;
    }

    const data = message.data;
    if (data.length < 2) {
      return;
    }

    const frames = data.length >> 1;
    if (this.queued + frames > this.maxQueued) {
      this.overruns += 1;
      while (this.chunks.length > 0 && this.queued + frames > this.maxQueued) {
        const dropped = this.chunks.shift();
        this.queued -= dropped.length >> 1;
        this.offset = 0;
      }
      if (this.queued + frames > this.maxQueued) {
        return;
      }
    }

    this.chunks.push(data);
    this.queued += frames;
    this.target = 1;
  }

  reset() {
    this.chunks = [];
    this.offset = 0;
    this.queued = 0;
    this.fade = 0;
    this.target = 0;
    this.wasEmpty = true;
  }

  process(_inputs, outputs) {
    const output = outputs[0];
    if (!output || output.length === 0) {
      return true;
    }

    const left = output[0];
    const right = output.length > 1 ? output[1] : null;
    const frames = left.length;
    const ramp = 1 / 128;

    for (let i = 0; i < frames; i += 1) {
      let sampleLeft = 0;
      let sampleRight = 0;

      if (this.queued > 0) {
        const chunk = this.chunks[0];
        if (chunk) {
          sampleLeft = chunk[this.offset];
          sampleRight = chunk[this.offset + 1];
          this.offset += 2;
          this.queued -= 1;
          if (this.offset >= chunk.length) {
            this.chunks.shift();
            this.offset = 0;
          }
          this.wasEmpty = false;
        }
      } else if (!this.wasEmpty) {
        this.underruns += 1;
        this.wasEmpty = true;
      }

      this.target = this.queued > 0 ? 1 : 0;
      if (this.fade < this.target) {
        this.fade = Math.min(this.target, this.fade + ramp);
      } else if (this.fade > this.target) {
        this.fade = Math.max(this.target, this.fade - ramp);
      }

      left[i] = sampleLeft * this.fade;
      if (right) {
        right[i] = sampleRight * this.fade;
      }
    }

    this.statCounter += frames;
    if (this.statCounter >= 2048) {
      this.statCounter = 0;
      this.port.postMessage({
        type: "stats",
        queued: this.queued,
        underruns: this.underruns,
        overruns: this.overruns,
      });
    }

    return true;
  }
}

registerProcessor("cinnabar-audio", CinnabarAudioProcessor);
