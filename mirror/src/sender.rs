use crate::FrameSinker;

use std::{
    mem::size_of,
    sync::{atomic::AtomicBool, Arc, Weak},
};

use anyhow::Result;
use bytes::BytesMut;
use capture::{
    AudioCaptureSourceDescription, Capture, CaptureDescriptor, FrameArrived, Source,
    SourceCaptureDescriptor, VideoCaptureSourceDescription,
};

use codec::{
    create_opus_identification_header, AudioEncoder, AudioEncoderSettings, VideoEncoder,
    VideoEncoderSettings, VideoEncoderType,
};

use frame::{AudioFrame, VideoFrame};
use transport::{
    adapter::{BufferFlag, StreamBufferInfo, StreamSenderAdapter},
    package,
};

use utils::{atomic::EasyAtomic, Size};

#[derive(Debug, Clone)]
pub struct VideoDescriptor {
    pub codec: VideoEncoderType,
    pub frame_rate: u8,
    pub width: u32,
    pub height: u32,
    pub bit_rate: u64,
    pub key_frame_interval: u32,
}

#[derive(Debug, Clone, Copy)]
pub struct AudioDescriptor {
    pub sample_rate: u64,
    pub bit_rate: u64,
}

#[derive(Debug)]
pub struct SenderDescriptor {
    pub video: Option<(Source, VideoDescriptor)>,
    pub audio: Option<(Source, AudioDescriptor)>,
    pub multicast: bool,
}

struct VideoSender {
    status: Weak<AtomicBool>,
    sink: Weak<dyn FrameSinker>,
    adapter: Weak<StreamSenderAdapter>,
    encoder: VideoEncoder,
}

impl VideoSender {
    // Encoding is a relatively complex task. If you add encoding tasks to the
    // pipeline that pushes frames, it will slow down the entire pipeline.
    //
    // Here, the tasks are separated, and the encoding tasks are separated into
    // independent threads. The encoding thread is notified of task updates through
    // the optional lock.
    fn new(
        status: &Arc<AtomicBool>,
        adapter: &Arc<StreamSenderAdapter>,
        settings: VideoEncoderSettings,
        sink: &Arc<dyn FrameSinker>,
    ) -> Result<Self> {
        Ok(Self {
            sink: Arc::downgrade(sink),
            adapter: Arc::downgrade(adapter),
            status: Arc::downgrade(status),
            encoder: VideoEncoder::new(settings)?,
        })
    }

    fn process(&mut self, frame: &VideoFrame) -> bool {
        // Push the audio and video frames into the encoder.
        if self.encoder.update(frame) {
            // Try to get the encoded data packets. The audio and video frames do not
            // correspond to the data packets one by one, so you need to try to get
            // multiple packets until they are empty.
            if let Err(e) = self.encoder.encode() {
                log::error!("video encode error={:?}", e);

                return false;
            } else {
                while let Some((buffer, flags, timestamp)) = self.encoder.read() {
                    if let Some(adapter) = self.adapter.upgrade() {
                        adapter.send(
                            package::copy_from_slice(buffer),
                            StreamBufferInfo::Video(flags, timestamp),
                        );
                    } else {
                        return false;
                    }
                }
            }
        } else {
            return false;
        }

        if let Some(sink) = self.sink.upgrade() {
            sink.video(frame)
        } else {
            false
        }
    }
}

impl FrameArrived for VideoSender {
    type Frame = VideoFrame;

    fn sink(&mut self, frame: &Self::Frame) -> bool {
        if self.process(frame) {
            true
        } else {
            if let (Some(status), Some(sink)) = (self.status.upgrade(), self.sink.upgrade()) {
                if !status.get() {
                    status.update(true);
                    sink.close();
                }
            }

            false
        }
    }
}

struct AudioSender {
    status: Weak<AtomicBool>,
    sink: Weak<dyn FrameSinker>,
    adapter: Weak<StreamSenderAdapter>,
    encoder: AudioEncoder,
    chunk_count: usize,
    buffer: BytesMut,
}

impl AudioSender {
    // Encoding is a relatively complex task. If you add encoding tasks to the
    // pipeline that pushes frames, it will slow down the entire pipeline.
    //
    // Here, the tasks are separated, and the encoding tasks are separated into
    // independent threads. The encoding thread is notified of task updates through
    // the optional lock.
    fn new(
        status: &Arc<AtomicBool>,
        adapter: &Arc<StreamSenderAdapter>,
        settings: AudioEncoderSettings,
        sink: &Arc<dyn FrameSinker>,
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
            chunk_count: settings.sample_rate as usize / 1000 * 100,
            encoder: AudioEncoder::new(settings)?,
            status: Arc::downgrade(status),
            adapter: Arc::downgrade(adapter),
            buffer: BytesMut::with_capacity(48000),
            sink: Arc::downgrade(sink),
        })
    }

    fn process(&mut self, frame: &AudioFrame) -> bool {
        self.buffer.extend_from_slice(unsafe {
            std::slice::from_raw_parts(
                frame.data as *const _,
                frame.frames as usize * size_of::<i16>(),
            )
        });

        if self.buffer.len() >= self.chunk_count * 2 {
            if let Some(adapter) = self.adapter.upgrade() {
                let payload = self.buffer.split_to(self.chunk_count * size_of::<i16>());
                let frame = AudioFrame {
                    data: payload.as_ptr() as *const _,
                    frames: self.chunk_count as u32,
                    sample_rate: 0,
                };

                if self.encoder.update(&frame) {
                    // Push the audio and video frames into the encoder.
                    if let Err(e) = self.encoder.encode() {
                        log::error!("audio encode error={:?}", e);

                        return false;
                    } else {
                        // Try to get the encoded data packets. The audio and video frames
                        // do not correspond to the data
                        // packets one by one, so you need to try to get
                        // multiple packets until they are empty.
                        while let Some((buffer, flags, timestamp)) = self.encoder.read() {
                            adapter.send(
                                package::copy_from_slice(buffer),
                                StreamBufferInfo::Audio(flags, timestamp),
                            );
                        }
                    }
                } else {
                    return false;
                }
            } else {
                return false;
            }
        }

        if let Some(sink) = self.sink.upgrade() {
            sink.audio(frame)
        } else {
            false
        }
    }
}

impl FrameArrived for AudioSender {
    type Frame = AudioFrame;

    fn sink(&mut self, frame: &Self::Frame) -> bool {
        if self.process(frame) {
            true
        } else {
            if let (Some(status), Some(sink)) = (self.status.upgrade(), self.sink.upgrade()) {
                if !status.get() {
                    status.update(true);
                    sink.close();
                }
            }

            false
        }
    }
}

pub struct Sender {
    pub(crate) adapter: Arc<StreamSenderAdapter>,
    status: Arc<AtomicBool>,
    sink: Arc<dyn FrameSinker>,
    capture: Capture,
}

impl Sender {
    // Create a sender. The capture of the sender is started following the sender,
    // but both video capture and audio capture can be empty, which means you can
    // create a sender that captures nothing.
    pub fn new<T: FrameSinker + 'static>(options: SenderDescriptor, sink: T) -> Result<Self> {
        log::info!("create sender");

        let mut capture_options = CaptureDescriptor::default();
        let adapter = StreamSenderAdapter::new(options.multicast);
        let status = Arc::new(AtomicBool::new(false));
        let sink: Arc<dyn FrameSinker> = Arc::new(sink);

        if let Some((source, options)) = options.audio {
            capture_options.audio = Some(SourceCaptureDescriptor {
                arrived: AudioSender::new(
                    &status,
                    &adapter,
                    AudioEncoderSettings {
                        sample_rate: options.sample_rate,
                        bit_rate: options.bit_rate,
                    },
                    &sink,
                )?,
                description: AudioCaptureSourceDescription {
                    sample_rate: options.sample_rate as u32,
                    source,
                },
            });
        }

        if let Some((source, options)) = options.video {
            capture_options.video = Some(SourceCaptureDescriptor {
                description: VideoCaptureSourceDescription {
                    hardware: codec::is_hardware_encoder(options.codec),
                    fps: options.frame_rate,
                    size: Size {
                        width: options.width,
                        height: options.height,
                    },
                    source,
                    #[cfg(target_os = "windows")]
                    direct3d: crate::DIRECT_3D_DEVICE
                        .read()
                        .unwrap()
                        .clone()
                        .expect("D3D device was not initialized successfully!"),
                },
                arrived: VideoSender::new(
                    &status,
                    &adapter,
                    VideoEncoderSettings {
                        codec: options.codec,
                        key_frame_interval: options.key_frame_interval,
                        frame_rate: options.frame_rate,
                        width: options.width,
                        height: options.height,
                        bit_rate: options.bit_rate,
                        #[cfg(target_os = "windows")]
                        direct3d: crate::DIRECT_3D_DEVICE.read().unwrap().clone(),
                    },
                    &sink,
                )?,
            });
        }

        Ok(Self {
            capture: Capture::new(capture_options)?,
            status,
            adapter,
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
        if let Err(e) = self.capture.close() {
            log::warn!("mirror sender capture close error={:?}", e);
        }

        self.adapter.close();
        if !self.status.get() {
            self.status.update(true);
            self.sink.close();
        }
    }
}
