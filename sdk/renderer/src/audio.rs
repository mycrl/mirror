use std::time::Duration;

use anyhow::Result;
use common::frame::AudioFrame;
use rodio::{OutputStream, OutputStreamHandle, Sink, Source};

pub struct AudioPlayer {
    /// Handle to a device that outputs sounds.
    ///
    /// Dropping the `Sink` stops all sounds. You can use `detach` if you want
    /// the sounds to continue playing.
    sink: Sink,
    /// `cpal::Stream` container. Also see the more useful `OutputStreamHandle`.
    ///
    /// If this is dropped playback will end & attached `OutputStreamHandle`s
    /// will no longer work.
    #[allow(unused)]
    stream: OutputStream,
    /// More flexible handle to a `OutputStream` that provides playback.
    #[allow(unused)]
    stream_handle: OutputStreamHandle,
}

impl AudioPlayer {
    pub fn new() -> Result<Self> {
        log::info!("create audio player");

        let (stream, stream_handle) = OutputStream::try_default()?;
        Ok(Self {
            sink: Sink::try_new(&stream_handle)?,
            stream_handle,
            stream,
        })
    }

    /// Push an audio clip to the queue.
    pub fn send(&self, sample_rate: u32, channels: u16, frame: &AudioFrame) {
        log::trace!(
            "append audio chunk to audio player, sample_rate={}, channels={}, frames={}",
            sample_rate,
            channels,
            frame.frames
        );

        self.sink.append(AudioBuffer {
            sample_rate,
            channels,
            index: 0,
            frames: frame.frames as usize,
            buffer: unsafe {
                std::slice::from_raw_parts(frame.data as *const i16, frame.frames as usize).to_vec()
            },
        })
    }
}

pub struct AudioBuffer {
    buffer: Vec<i16>,
    index: usize,
    channels: u16,
    sample_rate: u32,
    frames: usize,
}

impl Source for AudioBuffer {
    /// Returns the number of samples before the current frame ends. `None`
    /// means "infinite" or "until the sound ends".
    /// Should never return 0 unless there's no more data.
    ///
    /// After the engine has finished reading the specified number of samples,
    /// it will check whether the value of `channels()` and/or
    /// `sample_rate()` have changed.
    fn current_frame_len(&self) -> Option<usize> {
        Some(self.frames)
    }

    /// Returns the number of channels. Channels are always interleaved.
    fn channels(&self) -> u16 {
        self.channels
    }

    /// Returns the rate at which the source should be played. In number of
    /// samples per second.
    fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    /// Returns the total duration of this source, if known.
    ///
    /// `None` indicates at the same time "infinite" or "unknown".
    fn total_duration(&self) -> Option<Duration> {
        Some(Duration::from_millis(
            (self.frames as f64 / (self.sample_rate as f64 / 1000.0)) as u64,
        ))
    }
}

impl Iterator for AudioBuffer {
    type Item = i16;

    /// Read a value of a single sample.
    ///
    /// This trait is implemented by default on three types: `i16`, `u16` and
    /// `f32`.
    ///
    /// - For `i16`, silence corresponds to the value `0`. The minimum and
    ///   maximum amplitudes are represented by `i16::min_value()` and
    ///   `i16::max_value()` respectively.
    /// - For `u16`, silence corresponds to the value `u16::max_value() / 2`.
    ///   The minimum and maximum amplitudes are represented by `0` and
    ///   `u16::max_value()` respectively.
    /// - For `f32`, silence corresponds to the value `0.0`. The minimum and
    ///   maximum amplitudes are
    ///  represented by `-1.0` and `1.0` respectively.
    ///
    /// You can implement this trait on your own type as well if you wish so.
    fn next(&mut self) -> Option<Self::Item> {
        self.index += 1;
        self.buffer.get(self.index - 1).map(|it| *it)
    }
}
