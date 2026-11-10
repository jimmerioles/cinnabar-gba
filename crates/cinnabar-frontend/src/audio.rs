//! Audio output: a cpal stream fed through an rtrb SPSC ring buffer.
//!
//! The APU returns interleaved stereo `f32` samples at its own rate. The
//! frontend resamples them to the output device rate with a short linear
//! interpolator and maps them to however many channels the device wants.
//! Missing audio hardware is never fatal: the emulator keeps running silently.

use std::collections::VecDeque;

use anyhow::{Context, Result, bail};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{FromSample, SizedSample};

/// Enough samples for roughly a third of a second at 48 kHz stereo.
const RING_CAPACITY: usize = 32_768;

/// Owns the live output stream and the producer side of its sample queue.
pub struct AudioOutput {
    _stream: cpal::Stream,
    producer: rtrb::Producer<f32>,
    resampler: Option<LinearResampler>,
    staging: Vec<f32>,
}

impl AudioOutput {
    /// Open the default output device, or return `None` after logging once.
    pub fn start(source_rate: u32) -> Option<Self> {
        match Self::try_start(source_rate) {
            Ok(output) => Some(output),
            Err(error) => {
                eprintln!("cinnabar: audio unavailable, continuing without sound: {error:#}");
                None
            }
        }
    }

    fn try_start(source_rate: u32) -> Result<Self> {
        let host = cpal::default_host();
        let device = host
            .default_output_device()
            .context("no default audio output device")?;
        let supported = device
            .default_output_config()
            .context("failed to query the default audio output configuration")?;
        let sample_format = supported.sample_format();
        let config: cpal::StreamConfig = supported.into();
        let device_rate = config.sample_rate.0;
        let channels = config.channels as usize;
        let source_rate = source_rate.max(1);

        let (producer, consumer) = rtrb::RingBuffer::<f32>::new(RING_CAPACITY);
        let stream = match sample_format {
            cpal::SampleFormat::F32 => build_stream::<f32>(&device, &config, consumer, channels),
            cpal::SampleFormat::F64 => build_stream::<f64>(&device, &config, consumer, channels),
            cpal::SampleFormat::I8 => build_stream::<i8>(&device, &config, consumer, channels),
            cpal::SampleFormat::I16 => build_stream::<i16>(&device, &config, consumer, channels),
            cpal::SampleFormat::I32 => build_stream::<i32>(&device, &config, consumer, channels),
            cpal::SampleFormat::U8 => build_stream::<u8>(&device, &config, consumer, channels),
            cpal::SampleFormat::U16 => build_stream::<u16>(&device, &config, consumer, channels),
            cpal::SampleFormat::U32 => build_stream::<u32>(&device, &config, consumer, channels),
            other => bail!("unsupported audio sample format {other}"),
        }
        .context("failed to build the audio output stream")?;

        stream
            .play()
            .context("failed to start the audio output stream")?;

        eprintln!(
            "cinnabar: audio {:?} at {} Hz, {} channel(s)",
            sample_format, device_rate, channels
        );

        Ok(Self {
            _stream: stream,
            producer,
            resampler: (device_rate != source_rate)
                .then(|| LinearResampler::new(source_rate, device_rate)),
            staging: Vec::new(),
        })
    }

    /// Queue interleaved stereo `f32` samples from the APU.
    pub fn push(&mut self, interleaved_stereo: &[f32]) {
        if interleaved_stereo.is_empty() {
            return;
        }

        let samples = if let Some(resampler) = &mut self.resampler {
            self.staging.clear();
            resampler.process(interleaved_stereo, &mut self.staging);
            self.staging.as_slice()
        } else {
            interleaved_stereo
        };

        for &sample in samples {
            if self.producer.push(sample).is_err() {
                break;
            }
        }
    }
}

fn build_stream<T>(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    mut consumer: rtrb::Consumer<f32>,
    channels: usize,
) -> Result<cpal::Stream>
where
    T: SizedSample + FromSample<f32>,
{
    let channels = channels.max(1);
    let mut scratch: Vec<f32> = Vec::new();
    let mut error_reported = false;

    let stream = device
        .build_output_stream(
            config,
            move |data: &mut [T], _: &cpal::OutputCallbackInfo| {
                let frames = data.len() / channels;
                scratch.resize(frames * 2, 0.0);
                for sample in &mut scratch {
                    *sample = consumer.pop().unwrap_or(0.0);
                }

                for (frame_index, frame) in data.chunks_mut(channels).take(frames).enumerate() {
                    let left = scratch[frame_index * 2];
                    let right = scratch[frame_index * 2 + 1];
                    if channels == 1 {
                        frame[0] = T::from_sample((left + right) * 0.5);
                    } else {
                        for (channel, sample) in frame.iter_mut().enumerate() {
                            *sample = T::from_sample(if channel & 1 == 0 { left } else { right });
                        }
                    }
                }

                for sample in &mut data[frames * channels..] {
                    *sample = T::from_sample(0.0);
                }
            },
            move |error| {
                if !error_reported {
                    eprintln!("cinnabar: audio stream error: {error}");
                    error_reported = true;
                }
            },
            None,
        )
        .context("failed to open the audio output stream")?;

    Ok(stream)
}

/// Streaming linear resampler for interleaved stereo frames.
///
/// `position` is measured in source frames between `previous` and the front of
/// `pending`. One source frame is held back between calls, which keeps chunk
/// boundaries seamless.
struct LinearResampler {
    step: f64,
    position: f64,
    previous: [f32; 2],
    pending: VecDeque<[f32; 2]>,
    primed: bool,
}

impl LinearResampler {
    fn new(source_rate: u32, device_rate: u32) -> Self {
        Self {
            step: source_rate as f64 / device_rate as f64,
            position: 0.0,
            previous: [0.0; 2],
            pending: VecDeque::new(),
            primed: false,
        }
    }

    fn process(&mut self, input: &[f32], output: &mut Vec<f32>) {
        for frame in input.chunks_exact(2) {
            self.pending.push_back([frame[0], frame[1]]);
        }

        if !self.primed {
            let Some(first) = self.pending.pop_front() else {
                return;
            };
            self.previous = first;
            self.position = 0.0;
            self.primed = true;
        }

        loop {
            while self.position >= 1.0 {
                let Some(frame) = self.pending.pop_front() else {
                    return;
                };
                self.previous = frame;
                self.position -= 1.0;
            }

            let Some(next) = self.pending.front().copied() else {
                return;
            };
            let t = self.position as f32;
            output.push(lerp(self.previous[0], next[0], t));
            output.push(lerp(self.previous[1], next[1], t));
            self.position += self.step;
        }
    }
}

fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

#[cfg(test)]
mod tests {
    use super::*;

    fn interleaved(frames: &[[f32; 2]]) -> Vec<f32> {
        frames.iter().flat_map(|frame| *frame).collect()
    }

    #[test]
    fn lerp_endpoints() {
        assert_eq!(lerp(1.0, 3.0, 0.0), 1.0);
        assert_eq!(lerp(1.0, 3.0, 1.0), 3.0);
        assert_eq!(lerp(1.0, 3.0, 0.5), 2.0);
    }

    #[test]
    fn matching_rate_preserves_the_stream_over_chunks() {
        let mut resampler = LinearResampler::new(48_000, 48_000);
        let mut output = Vec::new();
        let first = interleaved(&[[0.0, 0.0], [1.0, -1.0], [2.0, -2.0]]);
        let second = interleaved(&[[3.0, -3.0], [4.0, -4.0]]);
        let third = interleaved(&[[5.0, -5.0]]);
        resampler.process(&first, &mut output);
        resampler.process(&second, &mut output);
        resampler.process(&third, &mut output);

        assert_eq!(output.len(), 10);
        let left: Vec<f32> = output.chunks_exact(2).map(|frame| frame[0]).collect();
        assert_eq!(left, vec![0.0, 1.0, 2.0, 3.0, 4.0]);
    }

    #[test]
    fn downsampling_uses_approximately_half_the_frames() {
        let mut resampler = LinearResampler::new(48_000, 24_000);
        let mut output = Vec::new();
        for _ in 0..10 {
            let input = interleaved(&[[1.0, 1.0]; 10]);
            resampler.process(&input, &mut output);
        }
        assert!((40..=60).contains(&(output.len() / 2)));
    }

    #[test]
    fn upsampling_uses_approximately_twice_the_frames() {
        let mut resampler = LinearResampler::new(24_000, 48_000);
        let mut output = Vec::new();
        for _ in 0..10 {
            let input = interleaved(&[[1.0, 1.0]; 10]);
            resampler.process(&input, &mut output);
        }
        assert!((180..=220).contains(&(output.len() / 2)));
    }
}
