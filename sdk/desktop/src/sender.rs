use crate::factory::FrameSink;

use std::sync::{Arc, Mutex, Weak};

use anyhow::Result;
use bytes::BytesMut;
use capture::{Capture, FrameArrived, Size, Source, SourceType, VideoCaptureSourceDescription};
use codec::{
    audio::create_opus_identification_header, AudioEncoder, AudioEncoderSettings, VideoEncoder,
    VideoEncoderSettings,
};

use common::frame::{AudioFormat, AudioFrame, VideoFrame};
use transport::{
    adapter::{BufferFlag, StreamBufferInfo, StreamSenderAdapter},
    package,
};

struct VideoSender {
    adapter: Weak<StreamSenderAdapter>,
    encoder: VideoEncoder,
    sink: Weak<FrameSink>,
}

impl FrameArrived for VideoSender {
    type Frame = VideoFrame;

    fn sink(&mut self, frame: &Self::Frame) -> bool {
        let ret = if let Some(adapter) = self.adapter.upgrade() {
            // Push the audio and video frames into the encoder.
            if self.encoder.send_frame(frame) {
                // Try to get the encoded data packets. The audio and video frames do not
                // correspond to the data packets one by one, so you need to try to get multiple
                // packets until they are empty.
                if self.encoder.encode() {
                    while let Some(packet) = self.encoder.read() {
                        adapter.send(
                            package::copy_from_slice(packet.buffer),
                            StreamBufferInfo::Video(packet.flags, packet.timestamp),
                        );
                    }
                }
            }

            true
        } else {
            if let Some(sink) = self.sink.upgrade() {
                (sink.close)();
            }

            false
        };

        if let Some(sink) = self.sink.upgrade() {
            (sink.video)(frame);
        }

        ret
    }
}

impl VideoSender {
    // Encoding is a relatively complex task. If you add encoding tasks to the
    // pipeline that pushes frames, it will slow down the entire pipeline.
    //
    // Here, the tasks are separated, and the encoding tasks are separated into
    // independent threads. The encoding thread is notified of task updates through
    // the optional lock.
    fn new(
        adapter: &Arc<StreamSenderAdapter>,
        settings: &VideoEncoderSettings,
        sink: &Arc<FrameSink>,
    ) -> Result<Self> {
        Ok(Self {
            encoder: VideoEncoder::new(settings)?,
            adapter: Arc::downgrade(adapter),
            sink: Arc::downgrade(sink),
        })
    }
}

struct AudioSender {
    sink: Weak<FrameSink>,
    adapter: Weak<StreamSenderAdapter>,
    buffer: Mutex<BytesMut>,
    encoder: AudioEncoder,
    frames: usize,
}

impl AudioSender {
    // Encoding is a relatively complex task. If you add encoding tasks to the
    // pipeline that pushes frames, it will slow down the entire pipeline.
    //
    // Here, the tasks are separated, and the encoding tasks are separated into
    // independent threads. The encoding thread is notified of task updates through
    // the optional lock.
    fn new(
        adapter: &Arc<StreamSenderAdapter>,
        settings: &AudioEncoderSettings,
        sink: &Arc<FrameSink>,
    ) -> Result<Self> {
        // Create an opus header data. The opus decoder needs this data to obtain audio
        // information. Here, actively add an opus header information to the queue, and
        // the transport layer will automatically cache it.
        adapter.send(
            package::copy_from_slice(&create_opus_identification_header(
                1,
                settings.sample_rate as u32,
            )),
            StreamBufferInfo::Audio(BufferFlag::Config as i32, 0),
        );

        Ok(AudioSender {
            sink: Arc::downgrade(sink),
            encoder: AudioEncoder::new(settings)?,
            adapter: Arc::downgrade(adapter),
            buffer: Mutex::new(BytesMut::with_capacity(48000)),
            frames: settings.sample_rate as usize / 1000 * 100,
        })
    }
}

impl FrameArrived for AudioSender {
    type Frame = AudioFrame;

    fn sink(&mut self, frame: &Self::Frame) -> bool {
        let ret = if let Some(adapter) = self.adapter.upgrade() {
            let mut buffer = self.buffer.lock().unwrap();
            buffer.extend_from_slice(unsafe {
                std::slice::from_raw_parts(frame.data, frame.frames as usize * 2)
            });

            if buffer.len() >= self.frames * 2 {
                let payload = buffer.split_to(self.frames * 2);
                let frame = AudioFrame {
                    format: AudioFormat::AUDIO_S16,
                    frames: self.frames as u32,
                    data: payload.as_ptr(),
                    sample_rate: 0,
                };

                if self.encoder.send_frame(&frame) {
                    // Push the audio and video frames into the encoder.
                    if self.encoder.encode() {
                        // Try to get the encoded data packets. The audio and video frames do not
                        // correspond to the data packets one by one, so you need to try to get
                        // multiple packets until they are empty.
                        while let Some(packet) = self.encoder.read() {
                            adapter.send(
                                package::copy_from_slice(packet.buffer),
                                StreamBufferInfo::Audio(packet.flags, packet.timestamp),
                            );
                        }
                    }
                }
            }

            true
        } else {
            if let Some(sink) = self.sink.upgrade() {
                (sink.close)();
            }

            false
        };

        if let Some(sink) = self.sink.upgrade() {
            (sink.audio)(frame);
        }

        ret
    }
}

#[derive(Debug)]
pub struct SenderOptions {
    pub video: VideoEncoderSettings,
    pub audio: AudioEncoderSettings,
    pub multicast: bool,
}

pub struct Sender {
    pub(crate) adapter: Arc<StreamSenderAdapter>,
    options: SenderOptions,
    sink: Arc<FrameSink>,
    capture: Capture,
}

impl Sender {
    pub fn new(options: SenderOptions, sink: FrameSink) -> Result<Self> {
        log::info!("create sender");

        Ok(Self {
            adapter: StreamSenderAdapter::new(options.multicast),
            capture: Capture::new()?,
            sink: Arc::new(sink),
            options,
        })
    }

    pub fn get_sources(&self, kind: SourceType) -> Result<Vec<Source>> {
        self.capture.get_sources(kind)
    }

    pub fn set_video_source(&self, source: Source) -> Result<()> {
        log::info!("sender set source={:?}", source);

        self.capture.set_video_source(
            VideoCaptureSourceDescription {
                fps: self.options.video.frame_rate,
                source,
                size: Size {
                    width: self.options.video.width,
                    height: self.options.video.height,
                },
            },
            VideoSender::new(&self.adapter, &self.options.video, &self.sink)?,
        )?;

        Ok(())
    }

    pub fn get_multicast(&self) -> bool {
        self.adapter.get_multicast()
    }

    pub fn set_multicast(&self, multicast: bool) {
        self.adapter.set_multicast(multicast)
    }
}

impl Drop for Sender {
    fn drop(&mut self) {
        log::info!("sender drop");

        self.adapter.close();
        (self.sink.close)()
    }
}
