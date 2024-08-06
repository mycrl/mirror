use crate::{AudioCaptureSourceDescription, CaptureHandler, Source, SourceType};

use std::sync::Mutex;

use anyhow::{anyhow, Result};
use common::frame::{AudioFrame, ReSampler};
use cpal::{traits::*, BufferSize, Host, Stream, StreamConfig};
use once_cell::sync::Lazy;

// Just use a default audio port globally.
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

    // Get the default input device. In theory, all microphones will be listed here.
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
        // Find devices with matching names
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
            move |data: &[i16], _| {
                // When any problem occurs in the process, you should not continue processing.
                // If the cpal bottom layer continues to push audio samples, it should be
                // ignored here and the process should not continue.
                if !playing {
                    return;
                }

                // Creating a resampler requires knowing the fixed number of input samples, but
                // in cpal the number of samples can only be known after the first frame is
                // obtained. There may be a question here, whether the number of
                // samples for each sample is fixed. It is currently observed that it is fixed,
                // so the default number of samples is fixed here.
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
                        frame.frames = sample.len() as u32;
                        frame.data = sample.as_ptr();

                        playing = arrived.sink(&frame);
                    }
                }
            },
            |e| {
                // An error has occurred, but there is nothing you can do at this moment except
                // output the error log.
                log::error!("audio capture callback error={:?}", e);
            },
            None,
        )?;

        stream.play()?;

        // If there is a previous stream, end it first.
        // Normally, a Capture instance is only used once, but here a defensive process
        // is done to avoid multiple calls due to external errors.
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
