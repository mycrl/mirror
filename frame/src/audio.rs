use std::ptr::null;

use rubato::{
    FastFixedIn, PolynomialDegree, ResampleResult, Resampler, ResamplerConstructionError,
};

/// Pulse-code modulation
///
/// Pulse-code modulation (PCM) is a method used to digitally represent analog
/// signals. It is the standard form of digital audio in computers, compact
/// discs, digital telephony and other digital audio applications. In a PCM
/// stream, the amplitude of the analog signal is sampled at uniform intervals,
/// and each sample is quantized to the nearest value within a range of digital
/// steps.
///
/// Linear pulse-code modulation (LPCM) is a specific type of PCM in which the
/// quantization levels are linearly uniform. This is in contrast to PCM
/// encodings in which quantization levels vary as a function of amplitude (as
/// with the A-law algorithm or the μ-law algorithm). Though PCM is a more
/// general term, it is often used to describe data encoded as LPCM.
///
/// A PCM stream has two basic properties that determine the stream's fidelity
/// to the original analog signal: the sampling rate, which is the number of
/// times per second that samples are taken; and the bit depth, which determines
/// the number of possible digital values that can be used to represent each
/// sample.
#[repr(C)]
#[derive(Debug)]
pub struct AudioFrame {
    pub sample_rate: u32,
    pub frames: u32,
    pub data: *const i16,
}

unsafe impl Sync for AudioFrame {}
unsafe impl Send for AudioFrame {}

impl Default for AudioFrame {
    fn default() -> Self {
        Self {
            frames: 0,
            data: null(),
            sample_rate: 0,
        }
    }
}

/// Audio resampler, quickly resample input to a single channel count and
/// different sampling rates.
///
/// Note that due to the fast sampling, the quality may be reduced.
pub struct AudioResampler {
    sampler: Option<FastFixedIn<f32>>,
    input_buffer: Vec<f32>,
    output_buffer: Vec<f32>,
    samples: Vec<i16>,
}

impl AudioResampler {
    pub fn new(input: f64, output: f64, frames: usize) -> Result<Self, ResamplerConstructionError> {
        Ok(Self {
            samples: Vec::with_capacity(frames),
            input_buffer: Vec::with_capacity(48000),
            output_buffer: vec![0.0; 48000],
            sampler: if input != output {
                Some(FastFixedIn::new(
                    output / input,
                    2.0,
                    PolynomialDegree::Linear,
                    frames,
                    1,
                )?)
            } else {
                None
            },
        })
    }

    pub fn resample<'a>(
        &'a mut self,
        buffer: &'a [i16],
        channels: usize,
    ) -> ResampleResult<&'a [i16]> {
        if channels == 1 && self.sampler.is_none() {
            Ok(buffer)
        } else {
            self.samples.clear();
            self.input_buffer.clear();

            for item in buffer.iter().step_by(channels) {
                if self.sampler.is_none() {
                    self.samples.push(*item);
                } else {
                    // need resample
                    self.input_buffer.push(*item as f32);
                }
            }

            if let Some(sampler) = &mut self.sampler {
                let (_, size) = sampler.process_into_buffer(
                    &[&self.input_buffer[..]],
                    &mut [&mut self.output_buffer],
                    None,
                )?;

                for item in &self.output_buffer[..size] {
                    self.samples.push(*item as i16);
                }
            }

            Ok(&self.samples[..])
        }
    }
}
