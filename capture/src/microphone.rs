use crate::{AudioCaptureSourceDescription, CaptureHandler, Source, SourceType};

use std::sync::Mutex;

use anyhow::{anyhow, Result};
use common::frame::{AudioFrame, ReSampler};
use cpal::{traits::*, BufferSize, Host, Stream, StreamConfig};
use once_cell::sync::Lazy;

static HOST: Lazy<Host> = Lazy::new(|| cpal::default_host());

#[derive(Default)]
pub struct MicrophoneCapture {
    stream: Mutex<Option<Stream>>,
}

unsafe impl Send for MicrophoneCapture {}
unsafe impl Sync for MicrophoneCapture {}

impl CaptureHandler for MicrophoneCapture {
    type Frame = AudioFrame;
    type Error = anyhow::Error;
    type CaptureOptions = AudioCaptureSourceDescription;

    fn get_sources() -> Result<Vec<Source>, Self::Error> {
        let mut sources = Vec::with_capacity(10);
        for (index, device) in HOST.input_devices()?.enumerate() {
            sources.push(Source {
                id: device.name()?,
                name: device.name()?,
                kind: SourceType::Microphone,
                index,
            });
        }

        Ok(sources)
    }

    fn start<S: crate::FrameArrived<Frame = Self::Frame> + 'static>(
        &self,
        options: Self::CaptureOptions,
        mut arrived: S,
    ) -> Result<(), Self::Error> {
        let device = HOST
            .input_devices()?
            .into_iter()
            .find(|it| {
                it.name()
                    .map(|name| name == options.source.name)
                    .unwrap_or(false)
            })
            .ok_or_else(|| anyhow!("not found the audio source"))?;

        // zero buffer size
        let mut config: StreamConfig = device.default_input_config()?.into();
        config.buffer_size = BufferSize::Fixed(0);

        let mut frame = AudioFrame::default();
        frame.sample_rate = options.sample_rate;

        let mut playing = true;
        let mut resampler = None;
        let stream = device.build_input_stream(
            &config,
            move |data: &[f32], _| {
                if !playing {
                    return;
                }

                if resampler.is_none() {
                    if let Ok(sampler) = ReSampler::new(
                        config.sample_rate.0 as f64,
                        options.sample_rate as f64,
                        data.len(),
                    ) {
                        resampler = Some(sampler);
                    }
                }

                if let Some(sampler) = &mut resampler {
                    if let Ok(sample) = sampler.resample(data, config.channels.into()) {
                        frame.data = sample.as_ptr() as *const _;
                        frame.frames = sample.len() as u32;

                        playing = arrived.sink(&frame);
                    }
                }
            },
            |e| {
                log::error!("audio capture callback error={:?}", e);
            },
            None,
        )?;

        stream.play()?;
        if let Some(stream) = self.stream.lock().unwrap().replace(stream) {
            stream.pause()?;
        }

        Ok(())
    }

    fn stop(&self) -> Result<(), Self::Error> {
        if let Some(stream) = self.stream.lock().unwrap().take() {
            stream.pause()?;
        }

        Ok(())
    }
}
