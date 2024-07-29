use std::{
    slice::from_raw_parts,
    sync::{
        mpsc::{channel, Receiver, Sender},
        Arc, RwLock,
    },
};

use anyhow::{anyhow, Result};
use common::frame::AudioFrame;
use cpal::{
    traits::{DeviceTrait, HostTrait, StreamTrait},
    SampleRate, Stream, StreamConfig, StreamError,
};

pub struct AudioPlayer {
    stream: Stream,
    config: StreamConfig,
    queue: Sender<SampleRateConverter>,
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

        // Start playing audio by default
        stream.play()?;

        Ok(Self {
            stream,
            queue,
            config,
            current_error,
        })
    }

    /// Push an audio clip to the queue.
    pub fn send(&mut self, frame: &AudioFrame) -> Result<()> {
        if let Some(current_error) = self.current_error.read().unwrap().as_ref() {
            return Err(anyhow!("{}", current_error));
        }

        self.queue.send(SampleRateConverter::new(
            unsafe { from_raw_parts(frame.data as *const i16, frame.frames as usize) }.to_vec(),
            SampleRate(frame.sample_rate),
            self.config.sample_rate,
            1,
        ))?;

        Ok(())
    }
}

impl Drop for AudioPlayer {
    fn drop(&mut self) {
        let _ = self.stream.pause();
    }
}

struct AudioQueue {
    queue: Receiver<SampleRateConverter>,
    current_chunk: Option<SampleRateConverter>,
}

static MUTE_BUF: [i16; 4800] = [0; 4800];

impl AudioQueue {
    fn read(&mut self, output: &mut [i16], channels: usize) {
        let mut index = 0;

        'a: while index < output.len() {
            if let Some(chunk) = &mut self.current_chunk {
                loop {
                    if index >= output.len() {
                        break;
                    }

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
                if let Ok(chunk) = self.queue.try_recv() {
                    self.current_chunk = Some(chunk);
                } else {
                    output.copy_from_slice(&MUTE_BUF[..output.len()]);
                    break;
                }
            }
        }
    }
}

/// Iterator that converts from a certain sample rate to another.
#[derive(Clone, Debug)]
pub struct SampleRateConverter {
    /// The iterator that gives us samples.
    input: std::vec::IntoIter<i16>,
    /// We convert chunks of `from` samples into chunks of `to` samples.
    from: u32,
    /// We convert chunks of `from` samples into chunks of `to` samples.
    to: u32,
    /// Number of channels in the stream
    channels: cpal::ChannelCount,
    /// One sample per channel, extracted from `input`.
    current_frame: Vec<i16>,
    /// Position of `current_sample` modulo `from`.
    current_frame_pos_in_chunk: u32,
    /// The samples right after `current_sample` (one per channel), extracted
    /// from `input`.
    next_frame: Vec<i16>,
    /// The position of the next sample that the iterator should return, modulo
    /// `to`. This counter is incremented (modulo `to`) every time the
    /// iterator is called.
    next_output_frame_pos_in_chunk: u32,
    /// The buffer containing the samples waiting to be output.
    output_buffer: Vec<i16>,
}

impl SampleRateConverter {
    ///
    ///
    /// # Panic
    ///
    /// Panics if `from` or `to` are equal to 0.
    #[inline]
    pub fn new(
        input: Vec<i16>,
        from: cpal::SampleRate,
        to: cpal::SampleRate,
        num_channels: cpal::ChannelCount,
    ) -> SampleRateConverter {
        let mut input = input.into_iter();
        let from = from.0;
        let to = to.0;

        assert!(from >= 1);
        assert!(to >= 1);

        // finding greatest common divisor
        let gcd = {
            #[inline]
            fn gcd(a: u32, b: u32) -> u32 {
                if b == 0 {
                    a
                } else {
                    gcd(b, a % b)
                }
            }

            gcd(from, to)
        };

        let (first_samples, next_samples) = if from == to {
            // if `from` == `to` == 1, then we just pass through
            debug_assert_eq!(from, gcd);
            (Vec::new(), Vec::new())
        } else {
            let first = input
                .by_ref()
                .take(num_channels as usize)
                .collect::<Vec<_>>();
            let next = input
                .by_ref()
                .take(num_channels as usize)
                .collect::<Vec<_>>();
            (first, next)
        };

        SampleRateConverter {
            input,
            from: from / gcd,
            to: to / gcd,
            channels: num_channels,
            current_frame_pos_in_chunk: 0,
            next_output_frame_pos_in_chunk: 0,
            current_frame: first_samples,
            next_frame: next_samples,
            output_buffer: Vec::with_capacity(num_channels as usize - 1),
        }
    }

    fn next_input_frame(&mut self) {
        self.current_frame_pos_in_chunk += 1;

        std::mem::swap(&mut self.current_frame, &mut self.next_frame);
        self.next_frame.clear();
        for _ in 0..self.channels {
            if let Some(i) = self.input.next() {
                self.next_frame.push(i);
            } else {
                break;
            }
        }
    }
}

impl Iterator for SampleRateConverter {
    type Item = i16;

    fn next(&mut self) -> Option<Self::Item> {
        // the algorithm below doesn't work if `self.from == self.to`
        if self.from == self.to {
            debug_assert_eq!(self.from, 1);
            return self.input.next();
        }

        // Short circuit if there are some samples waiting.
        if !self.output_buffer.is_empty() {
            return Some(self.output_buffer.remove(0));
        }

        // The frame we are going to return from this function will be a linear
        // interpolation between `self.current_frame` and `self.next_frame`.

        if self.next_output_frame_pos_in_chunk == self.to {
            // If we jump to the next frame, we reset the whole state.
            self.next_output_frame_pos_in_chunk = 0;

            self.next_input_frame();
            while self.current_frame_pos_in_chunk != self.from {
                self.next_input_frame();
            }
            self.current_frame_pos_in_chunk = 0;
        } else {
            // Finding the position of the first sample of the linear interpolation.
            let req_left_sample =
                (self.from * self.next_output_frame_pos_in_chunk / self.to) % self.from;

            // Advancing `self.current_frame`, `self.next_frame` and
            // `self.current_frame_pos_in_chunk` until the latter variable
            // matches `req_left_sample`.
            while self.current_frame_pos_in_chunk != req_left_sample {
                self.next_input_frame();
                debug_assert!(self.current_frame_pos_in_chunk < self.from);
            }
        }

        // Merging `self.current_frame` and `self.next_frame` into `self.output_buffer`.
        // Note that `self.output_buffer` can be truncated if there is not enough data
        // in `self.next_frame`.
        let mut result = None;
        let numerator = (self.from * self.next_output_frame_pos_in_chunk) % self.to;
        for (off, (cur, next)) in self
            .current_frame
            .iter()
            .zip(self.next_frame.iter())
            .enumerate()
        {
            let sample = {
                (*cur as i32 + (*next as i32 - *cur as i32) * numerator as i32 / self.to as i32)
                    as i16
            };

            if off == 0 {
                result = Some(sample);
            } else {
                self.output_buffer.push(sample);
            }
        }

        // Incrementing the counter for the next iteration.
        self.next_output_frame_pos_in_chunk += 1;

        if result.is_some() {
            result
        } else {
            // draining `self.current_frame`
            if !self.current_frame.is_empty() {
                let r = Some(self.current_frame.remove(0));
                std::mem::swap(&mut self.output_buffer, &mut self.current_frame);
                self.current_frame.clear();
                r
            } else {
                None
            }
        }
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        let apply = |samples: usize| {
            // `samples_after_chunk` will contain the number of samples remaining after the
            // chunk currently being processed
            let samples_after_chunk = samples;
            // adding the samples of the next chunk that may have already been read
            let samples_after_chunk = if self.current_frame_pos_in_chunk == self.from - 1 {
                samples_after_chunk + self.next_frame.len()
            } else {
                samples_after_chunk
            };
            // removing the samples of the current chunk that have not yet been read
            let samples_after_chunk = samples_after_chunk.saturating_sub(
                self.from
                    .saturating_sub(self.current_frame_pos_in_chunk + 2) as usize
                    * usize::from(self.channels),
            );
            // calculating the number of samples after the transformation
            // TODO: this is wrong here \|/
            let samples_after_chunk = samples_after_chunk * self.to as usize / self.from as usize;

            // `samples_current_chunk` will contain the number of samples remaining to be
            // output for the chunk currently being processed
            let samples_current_chunk = (self.to - self.next_output_frame_pos_in_chunk) as usize
                * usize::from(self.channels);

            samples_current_chunk + samples_after_chunk + self.output_buffer.len()
        };

        if self.from == self.to {
            self.input.size_hint()
        } else {
            let (min, max) = self.input.size_hint();
            (apply(min), max.map(apply))
        }
    }
}
