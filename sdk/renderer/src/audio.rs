use std::{slice::from_raw_parts, time::Duration};

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
        log::info!("renderer: create audio player");

        let (stream, stream_handle) = OutputStream::try_default()?;
        Ok(Self {
            sink: Sink::try_new(&stream_handle)?,
            stream_handle,
            stream,
        })
    }

    /// Push an audio clip to the queue.
    pub fn send(&mut self, frame: &AudioFrame) {
        let buffer = unsafe { from_raw_parts(frame.data as *const i16, frame.frames as usize) };
        self.sink.append(AudioSource {
            sample_rate: frame.sample_rate,
            buffer: buffer.to_vec(),
        })
    }
}

struct AudioSource {
    buffer: Vec<i16>,
    /// A sound is a vibration that propagates through air and reaches your
    /// ears. This vibration can be represented as an analog signal.
    ///
    /// In order to store this signal in the computer’s memory or on the disk,
    /// we perform what is called sampling. The consists in choosing an interval
    /// of time (for example 20µs) and reading the amplitude of the signal at
    /// each interval (for example, if the interval is 20µs we read the
    /// amplitude every 20µs). By doing so we obtain a list of numerical values,
    /// each value being called a sample.
    ///
    /// Therefore a sound can be represented in memory by a frequency and a list
    /// of samples. The frequency is expressed in hertz and corresponds to the
    /// number of samples that have been read per second. For example if we read
    /// one sample every 20µs, the frequency would be 50000 Hz. In reality,
    /// common values for the frequency are 44100, 48000 and 96000.
    sample_rate: u32,
}

impl Source for AudioSource {
    /// Returns the number of samples before the current frame ends. `None`
    /// means "infinite" or "until the sound ends".
    /// Should never return 0 unless there's no more data.
    ///
    /// After the engine has finished reading the specified number of samples,
    /// it will check whether the value of `channels()` and/or
    /// `sample_rate()` have changed.
    fn current_frame_len(&self) -> Option<usize> {
        None
    }

    /// Returns the number of channels. Channels are always interleaved.
    fn channels(&self) -> u16 {
        1
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
        None
    }
}

impl Iterator for AudioSource {
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
        self.buffer.pop()
    }
}
