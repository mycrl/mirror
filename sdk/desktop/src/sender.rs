use crate::factory::FrameSink;

use std::{
    mem::size_of,
    sync::{Arc, Mutex, Weak},
};

use anyhow::Result;
use bytes::BytesMut;
use capture::{
    AudioCaptureSourceDescription, Capture, FrameArrived, Size, Source,
    VideoCaptureSourceDescription,
};

use codec::{
    audio::create_opus_identification_header, AudioEncoder, AudioEncoderSettings, VideoEncoder,
    VideoEncoderSettings,
};

use frame::{AudioFrame, VideoFrame};
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
            // This is a rather strange implementation, but it is very useful and will have
            // an impact on reducing latency.
            //
            // Why is it implemented this way? This is because the internal audio encoder
            // has several specific limits on the number of samples submitted at a time, but
            // I cannot control the size of the external single submission, so there is a
            // 100 millisecond buffer here, and when the buffer is full, it is submitted to
            // the encoder to ensure that a fixed number of audio samples are submitted each
            // time.
            let mut buffer = self.buffer.lock().unwrap();
            buffer.extend_from_slice(unsafe {
                std::slice::from_raw_parts(
                    frame.data as *const _,
                    frame.frames as usize * size_of::<f32>(),
                )
            });

            if buffer.len() >= self.frames * 2 {
                let payload = buffer.split_to(self.frames * 2);
                let frame = AudioFrame {
                    data: payload.as_ptr() as *const _,
                    frames: self.frames as u32,
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
    pub video: Option<(Source, VideoEncoderSettings)>,
    pub audio: Option<(Source, AudioEncoderSettings)>,
    pub multicast: bool,
}

pub struct Sender {
    pub(crate) adapter: Arc<StreamSenderAdapter>,
    sink: Arc<FrameSink>,
    capture: Capture,
}

impl Sender {
    // Create a sender. The capture of the sender is started following the sender,
    // but both video capture and audio capture can be empty, which means you can
    // create a sender that captures nothing.
    pub fn new(options: SenderOptions, sink: FrameSink) -> Result<Self> {
        log::info!("create sender");

        let adapter = StreamSenderAdapter::new(options.multicast);
        let capture = Capture::default();
        let sink = Arc::new(sink);

        if let Some((source, options)) = options.audio {
            let codec = AudioSender::new(&adapter, &options, &sink)?;
            capture.set_audio_source(
                AudioCaptureSourceDescription {
                    sample_rate: options.sample_rate as u32,
                    source,
                },
                codec,
            )?;
        }

        if let Some((source, options)) = options.video {
            let codec = VideoSender::new(&adapter, &options, &sink)?;
            capture.set_video_source(
                VideoCaptureSourceDescription {
                    fps: options.frame_rate,
                    source,
                    size: Size {
                        width: options.width,
                        height: options.height,
                    },
                },
                codec,
            )?;
        }

        Ok(Self {
            adapter,
            capture,
            sink,
        })
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

        // When the sender releases, the cleanup work should be done, but there is a
        // more troublesome point here. If it is actively released by the outside, it
        // will also call back to the external closing event. It stands to reason that
        // it should be distinguished whether it is an active closure, but in order to
        // make it simpler to implement, let's do it this way first.
        let _ = self.capture.close();
        self.adapter.close();
        (self.sink.close)()
    }
}
