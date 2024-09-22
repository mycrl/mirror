use std::{
    slice::from_raw_parts,
    sync::{
        mpsc::{channel, Receiver, Sender},
        Arc, RwLock,
    },
};

use anyhow::{anyhow, Result};
use audio::AudioResampler;
use cpal::{
    traits::{DeviceTrait, HostTrait, StreamTrait},
    Stream, StreamConfig, StreamError,
};

use frame::AudioFrame;

pub struct AudioPlayer {
    stream: Stream,
    config: StreamConfig,
    queue: Sender<Vec<i16>>,
    sampler: Option<AudioResampler>,
    current_error: Arc<RwLock<Option<StreamError>>>,
}

impl AudioPlayer {
    pub fn new() -> Result<Self> {
        let host = cpal::default_host();
        let device = host
            .default_output_device()
            .ok_or_else(|| anyhow!("no output device available"))?;
        let config: StreamConfig = device.default_output_config()?.into();
        let current_error: Arc<RwLock<Option<StreamError>>> = Default::default();

        let (queue, rx) = channel();
        let stream = {
            let current_error_ = Arc::downgrade(&current_error);
            let mut queue = AudioQueue {
                queue: rx,
                current_chunk: None,
            };

            device.build_output_stream(
                &config,
                move |data: &mut [i16], _: &cpal::OutputCallbackInfo| {
                    queue.read(data, config.channels as usize);
                },
                move |err| {
                    if let Some(current_error) = current_error_.upgrade() {
                        current_error.write().unwrap().replace(err);
                    }
                },
                None,
            )?
        };

        Ok(Self {
            stream,
            queue,
            config,
            current_error,
            sampler: None,
        })
    }

    /// Push an audio clip to the queue.
    pub fn send(&mut self, frame: &AudioFrame) -> Result<()> {
        if let Some(current_error) = self.current_error.read().unwrap().as_ref() {
            return Err(anyhow!("{}", current_error));
        }

        if self.sampler.is_none() {
            self.sampler = Some(AudioResampler::new(
                frame.sample_rate as f64,
                self.config.sample_rate.0 as f64,
                frame.frames as usize,
            )?);

            // Start playing audio by first push.
            self.stream.play()?;
        }

        if let Some(sampler) = &mut self.sampler {
            self.queue.send(
                sampler
                    .resample(
                        unsafe { from_raw_parts(frame.data, frame.frames as usize) },
                        1,
                    )?
                    .to_vec(),
            )?;
        }

        Ok(())
    }
}

impl Drop for AudioPlayer {
    fn drop(&mut self) {
        let _ = self.stream.pause();
    }
}

struct AudioQueue {
    queue: Receiver<Vec<i16>>,
    current_chunk: Option<std::vec::IntoIter<i16>>,
}

static MUTE_BUF: [i16; 48000] = [0; 48000];

impl AudioQueue {
    fn read(&mut self, output: &mut [i16], channels: usize) {
        let mut index = 0;

        // Copy from queue to player
        'a: while index < output.len() {
            // Check if the buffer is empty
            if let Some(chunk) = &mut self.current_chunk {
                loop {
                    // Writing to the player buffer is complete
                    if index >= output.len() {
                        break;
                    }

                    // Read data from the queue buffer and write it to the player buffer. If the
                    // queue buffer is empty, jump to the step of updating the buffer.
                    if let Some(item) = chunk.next() {
                        for i in 0..channels {
                            output[index + i] = item;
                        }

                        index += channels;
                    } else {
                        self.current_chunk = None;
                        continue 'a;
                    }
                }
            } else {
                // If the buffer is empty, take another one from the queue and put it into the
                // buffer. If the queue is empty, fill it directly with silent data.
                if let Ok(chunk) = self.queue.try_recv() {
                    self.current_chunk = Some(chunk.into_iter());
                } else {
                    output.copy_from_slice(&MUTE_BUF[..output.len()]);
                    break;
                }
            }
        }
    }
}
