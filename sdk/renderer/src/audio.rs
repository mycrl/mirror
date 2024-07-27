use std::{
    collections::LinkedList,
    slice::from_raw_parts,
    sync::{Arc, Mutex},
};

use anyhow::{anyhow, Result};
use common::frame::AudioFrame;
use cpal::{
    traits::{DeviceTrait, HostTrait, StreamTrait},
    Data, Device, Host, SampleFormat, Stream, StreamConfig,
};
use rubato::{FftFixedIn, Resampler};

pub struct AudioPlayer {
    host: Host,
    device: Device,
    stream: Stream,
    config: StreamConfig,
    queue: Arc<AudioQueue>,
    sampler: Option<FftFixedIn<f32>>,
    buffer: Vec<f32>,
}

impl AudioPlayer {
    pub fn new() -> Result<Self> {
        let host = cpal::default_host();
        let device = host
            .default_output_device()
            .ok_or_else(|| anyhow!("no output device available"))?;
        let config: StreamConfig = device.default_output_config()?.into();
        let queue = Arc::new(AudioQueue::default());

        let queue_ = Arc::downgrade(&queue);
        let stream = device.build_output_stream_raw(
            &config,
            SampleFormat::F32,
            move |data: &mut Data, _: &cpal::OutputCallbackInfo| {
                println!("==================== {:#?}", data.sample_format());
                if let Some(queue) = queue_.upgrade() {
                    let chunk_size = data.len() / config.channels as usize;
                    queue.read(&mut data.as_slice_mut().unwrap()[..chunk_size]);
                }
            },
            |err| {},
            None,
        )?;

        stream.play()?;
        Ok(Self {
            host,
            device,
            stream,
            queue,
            config,
            sampler: None,
            buffer: Vec::with_capacity(48000),
        })
    }

    /// Push an audio clip to the queue.
    pub fn send(&mut self, frame: &AudioFrame) -> Result<()> {
        if self.sampler.is_none() {
            self.sampler.replace(FftFixedIn::<f32>::new(
                frame.sample_rate as usize,
                self.config.sample_rate.0 as usize,
                frame.frames as usize,
                2,
                1,
            )?);
        }

        self.buffer.clear();

        for item in unsafe { from_raw_parts(frame.data as *const i16, frame.frames as usize) } {
            self.buffer.push(*item as f32);
        }

        if let Some(sampler) = self.sampler.as_mut() {
            let mut chunk = AudioChunk {
                bytes: vec![0.0; 48000 / 2],
                offset: 0,
            };

            let (_, size) =
                sampler.process_into_buffer(&[&self.buffer], &mut [&mut chunk.bytes], None)?;
            unsafe { chunk.bytes.set_len(size) }
            self.queue.push_chunk(chunk);
        }

        Ok(())
    }
}

struct AudioChunk {
    bytes: Vec<f32>,
    offset: usize,
}

#[derive(Default)]
struct AudioQueue(Mutex<LinkedList<AudioChunk>>);

impl AudioQueue {
    fn push_chunk(&self, chunk: AudioChunk) {
        self.0.lock().unwrap().push_front(chunk)
    }

    fn read(&self, output: &mut [f32]) {
        let mut queue = self.0.lock().unwrap();
        if queue.is_empty() {
            return;
        }

        let mut size = 0;
        let count = output.len();

        while size < count {
            let dst = &mut output[size..count];
            if let Some(chunk) = queue.back_mut() {
                if chunk.bytes.len() <= chunk.offset {
                    queue.pop_back();
                    continue;
                }

                let src = &chunk.bytes[chunk.offset..];
                let need = if src.len() > dst.len() {
                    dst.len()
                } else {
                    src.len()
                };

                dst[..need].copy_from_slice(&src[..need]);
                chunk.offset += need;
                size += need;
            } else {
                break;
            }
        }
    }
}
