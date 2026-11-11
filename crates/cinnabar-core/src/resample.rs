//! Streaming audio resampling shared by the native and browser frontends.
//!
//! The APU produces interleaved stereo samples at 48 kHz. Audio devices do
//! not always run at that rate, so both frontends resample into the output
//! device's clock. The cubic interpolator keeps state across calls, which
//! makes chunk boundaries seamless.

use std::collections::VecDeque;

/// Streaming cubic resampler for interleaved stereo frames.
///
/// `position` is measured in source frames between `p1` and `p2`. Two future
/// frames are held back between calls, which keeps chunk boundaries seamless.
pub struct CubicResampler {
    step: f64,
    position: f64,
    p0: [f32; 2],
    p1: [f32; 2],
    pending: VecDeque<[f32; 2]>,
    primed: bool,
}

impl CubicResampler {
    pub fn new(source_rate: u32, output_rate: u32) -> Self {
        let source_rate = source_rate.max(1);
        let output_rate = output_rate.max(1);
        Self {
            step: f64::from(source_rate) / f64::from(output_rate),
            position: 0.0,
            p0: [0.0; 2],
            p1: [0.0; 2],
            pending: VecDeque::new(),
            primed: false,
        }
    }

    /// Drop all buffered history, for example after a reset or a rate change.
    pub fn reset(&mut self) {
        self.position = 0.0;
        self.p0 = [0.0; 2];
        self.p1 = [0.0; 2];
        self.pending.clear();
        self.primed = false;
    }

    /// Resample interleaved stereo `input` and append the result to `output`.
    pub fn process(&mut self, input: &[f32], output: &mut Vec<f32>) {
        for frame in input.chunks_exact(2) {
            self.pending.push_back([frame[0], frame[1]]);
        }

        if !self.primed {
            if self.pending.len() < 2 {
                return;
            }
            self.p0 = self.pending.pop_front().unwrap_or([0.0; 2]);
            self.p1 = self.pending.pop_front().unwrap_or([0.0; 2]);
            self.position = 0.0;
            self.primed = true;
        }

        loop {
            while self.position >= 1.0 {
                let Some(frame) = self.pending.pop_front() else {
                    return;
                };
                self.p0 = self.p1;
                self.p1 = frame;
                self.position -= 1.0;
            }

            let Some(p2) = self.pending.front().copied() else {
                return;
            };
            let Some(p3) = self.pending.get(1).copied() else {
                return;
            };
            let t = self.position as f32;
            output.push(cubic(self.p0[0], self.p1[0], p2[0], p3[0], t));
            output.push(cubic(self.p0[1], self.p1[1], p2[1], p3[1], t));
            self.position += self.step;
        }
    }
}

fn cubic(p0: f32, p1: f32, p2: f32, p3: f32, t: f32) -> f32 {
    0.5 * (2.0 * p1
        + (-p0 + p2) * t
        + (2.0 * p0 - 5.0 * p1 + 4.0 * p2 - p3) * t * t
        + (-p0 + 3.0 * p1 - 3.0 * p2 + p3) * t * t * t)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn interleaved(frames: &[[f32; 2]]) -> Vec<f32> {
        frames.iter().flat_map(|frame| *frame).collect()
    }

    #[test]
    fn cubic_passes_through_linear_points() {
        assert_eq!(cubic(0.0, 1.0, 2.0, 3.0, 0.0), 1.0);
        assert!((cubic(0.0, 1.0, 2.0, 3.0, 0.5) - 1.5).abs() < 0.001);
    }

    #[test]
    fn matching_rate_drops_the_startup_and_tail_frames() {
        let mut resampler = CubicResampler::new(48_000, 48_000);
        let mut output = Vec::new();
        let first = interleaved(&[[0.0, 0.0], [1.0, -1.0], [2.0, -2.0]]);
        let second = interleaved(&[[3.0, -3.0], [4.0, -4.0]]);
        let third = interleaved(&[[5.0, -5.0]]);
        resampler.process(&first, &mut output);
        resampler.process(&second, &mut output);
        resampler.process(&third, &mut output);

        let left: Vec<f32> = output.chunks_exact(2).map(|frame| frame[0]).collect();
        assert_eq!(left, vec![1.0, 2.0, 3.0]);
    }

    #[test]
    fn downsampling_uses_approximately_half_the_frames() {
        let mut resampler = CubicResampler::new(48_000, 24_000);
        let mut output = Vec::new();
        for _ in 0..10 {
            let input = interleaved(&[[1.0, 1.0]; 10]);
            resampler.process(&input, &mut output);
        }
        assert!((39..=60).contains(&(output.len() / 2)));
    }

    #[test]
    fn upsampling_uses_approximately_twice_the_frames() {
        let mut resampler = CubicResampler::new(24_000, 48_000);
        let mut output = Vec::new();
        for _ in 0..10 {
            let input = interleaved(&[[1.0, 1.0]; 10]);
            resampler.process(&input, &mut output);
        }
        assert!((180..=220).contains(&(output.len() / 2)));
    }

    #[test]
    fn reset_drops_pending_history() {
        let mut resampler = CubicResampler::new(48_000, 24_000);
        let mut output = Vec::new();
        resampler.process(&interleaved(&[[1.0, 1.0]; 3]), &mut output);
        resampler.reset();
        output.clear();
        resampler.process(&interleaved(&[[1.0, 1.0]; 2]), &mut output);
        assert!(output.is_empty());
    }
}
