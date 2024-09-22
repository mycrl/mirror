use rubato::{
    FastFixedIn, PolynomialDegree, ResampleResult, Resampler, ResamplerConstructionError,
};

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
