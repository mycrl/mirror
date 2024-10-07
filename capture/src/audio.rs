use crate::{AudioCaptureSourceDescription, CaptureHandler, Source, SourceType};

use parking_lot::Mutex;

use anyhow::{anyhow, Result};
use common::frame::AudioFrame;
use cpal::{traits::*, Host, Stream, StreamConfig};
use once_cell::sync::Lazy;
use resample::AudioResampler;

// Just use a default audio port globally.
static HOST: Lazy<Host> = Lazy::new(|| cpal::default_host());

enum DeviceKind {
    Input,
    Output,
}

#[derive(Default)]
pub struct AudioCapture(Mutex<Option<Stream>>);

unsafe impl Send for AudioCapture {}
unsafe impl Sync for AudioCapture {}

impl CaptureHandler for AudioCapture {
    type Frame = AudioFrame;
    type Error = anyhow::Error;
    type CaptureDescriptor = AudioCaptureSourceDescription;

    // Get the default input device. In theory, all microphones will be listed here.
    fn get_sources() -> Result<Vec<Source>, Self::Error> {
        let default_name = HOST
            .default_output_device()
            .map(|it| it.name().ok())
            .flatten();

        // If you ever need to switch back to recording, you just need to capture the
        // output device, which is really funny, but very simple and worth mentioning!
        let mut sources = Vec::with_capacity(20);
        for (index, device) in HOST
            .output_devices()?
            .chain(HOST.input_devices()?)
            .enumerate()
        {
            sources.push(Source {
                id: device.name()?,
                name: device.name()?,
                kind: SourceType::Audio,
                is_default: device.name().ok() == default_name,
                index,
            });
        }

        Ok(sources)
    }

    fn start<S: crate::FrameArrived<Frame = Self::Frame> + 'static>(
        &self,
        options: Self::CaptureDescriptor,
        mut arrived: S,
    ) -> Result<(), Self::Error> {
        // Find devices with matching names
        let (device, kind) = HOST
            .output_devices()?
            .map(|it| (it, DeviceKind::Output))
            .chain(HOST.input_devices()?.map(|it| (it, DeviceKind::Input)))
            .find(|(it, _)| {
                it.name()
                    .map(|name| name == options.source.name)
                    .unwrap_or(false)
            })
            .ok_or_else(|| anyhow!("not found the audio source"))?;

        let config: StreamConfig = match kind {
            DeviceKind::Input => device.default_input_config()?.into(),
            DeviceKind::Output => device.default_output_config()?.into(),
        };

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
                    if let Ok(sampler) = AudioResampler::new(
                        config.sample_rate.0 as f64,
                        options.sample_rate as f64,
                        data.len() / config.channels as usize,
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
        if let Some(stream) = self.0.lock().replace(stream) {
            stream.pause()?;
        }

        Ok(())
    }

    fn stop(&self) -> Result<(), Self::Error> {
        if let Some(stream) = self.0.lock().take() {
            stream.pause()?;
        }

        Ok(())
    }
}
