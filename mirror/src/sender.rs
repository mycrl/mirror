use crate::FrameSink;

use std::{
    mem::size_of,
    sync::{Arc, Mutex, Weak},
    thread,
};

use anyhow::Result;
use bytes::BytesMut;
use capture::{
    AudioCaptureSourceDescription, Capture, CaptureDescriptor, FrameArrived, Size, Source,
    SourceCaptureDescriptor, VideoCaptureSourceDescription,
};

use codec::{
    create_opus_identification_header, AudioEncoder, AudioEncoderSettings, VideoEncoder,
    VideoEncoderSettings, VideoEncoderType,
};

use crossbeam::sync::{Parker, Unparker};
use frame::{AudioFrame, VideoFrame};
use transport::{
    adapter::{BufferFlag, StreamBufferInfo, StreamSenderAdapter},
    package,
};

#[cfg(target_os = "windows")]
use utils::win32::MediaThreadClass;

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
    adapter: Weak<StreamSenderAdapter>,
    encoder: VideoEncoder,
    sink: Weak<FrameSink>,
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
        settings: VideoEncoderSettings,
        sink: &Arc<FrameSink>,
    ) -> Result<Self> {
        Ok(Self {
            sink: Arc::downgrade(sink),
            adapter: Arc::downgrade(adapter),
            encoder: VideoEncoder::new(settings)?,
        })
    }
}

impl FrameArrived for VideoSender {
    type Frame = VideoFrame;

    fn sink(&mut self, frame: &Self::Frame) -> bool {
        // // Push the audio and video frames into the encoder.
        // if self.encoder.update(frame) {
        //     // Try to get the encoded data packets. The audio and video frames do not
        //     // correspond to the data packets one by one, so you need to try to get
        //     // multiple packets until they are empty.
        //     if let Err(e) = self.encoder.encode() {
        //         log::error!("video encode error={:?}", e);

        //         return false;
        //     } else {
        //         while let Some((buffer, flags, timestamp)) = self.encoder.read() {
        //             if let Some(adapter) = self.adapter.upgrade() {
        //                 adapter.send(
        //                     package::copy_from_slice(buffer),
        //                     StreamBufferInfo::Video(flags, timestamp),
        //                 );
        //             } else {
        //                 return false;
        //             }
        //         }
        //     }
        // } else {
        //     return false;
        // }

        if let Some(sink) = self.sink.upgrade() {
            (sink.video)(frame);
        }

        true
    }
}

struct AudioSender {
    buffer: Arc<Mutex<BytesMut>>,
    sink: Weak<FrameSink>,
    unparker: Unparker,
    chunk_count: usize,
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
        settings: AudioEncoderSettings,
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

        let parker = Parker::new();
        let unparker = parker.unparker().clone();
        let mut encoder = AudioEncoder::new(settings)?;
        let buffer = Arc::new(Mutex::new(BytesMut::with_capacity(48000)));
        let chunk_count = settings.sample_rate as usize / 1000 * 100;

        let sink_ = Arc::downgrade(sink);
        let buffer_ = Arc::downgrade(&buffer);
        let adapter_ = Arc::downgrade(adapter);
        thread::Builder::new()
            .name("AudioEncoderThread".to_string())
            .spawn(move || {
                #[cfg(target_os = "windows")]
                let thread_class_guard = MediaThreadClass::ProAudio.join().ok();

                loop {
                    parker.park();

                    if let (Some(adapter), Some(buffer)) = (adapter_.upgrade(), buffer_.upgrade()) {
                        let payload = buffer
                            .lock()
                            .unwrap()
                            .split_to(chunk_count * size_of::<i16>());
                        let frame = AudioFrame {
                            data: payload.as_ptr() as *const _,
                            frames: chunk_count as u32,
                            sample_rate: 0,
                        };

                        if encoder.update(&frame) {
                            // Push the audio and video frames into the encoder.
                            if let Err(e) = encoder.encode() {
                                log::error!("audio encode error={:?}", e);

                                break;
                            } else {
                                // Try to get the encoded data packets. The audio and video frames
                                // do not correspond to the data
                                // packets one by one, so you need to try to get
                                // multiple packets until they are empty.
                                while let Some((buffer, flags, timestamp)) = encoder.read() {
                                    adapter.send(
                                        package::copy_from_slice(buffer),
                                        StreamBufferInfo::Audio(flags, timestamp),
                                    );
                                }
                            }
                        } else {
                            break;
                        }
                    } else {
                        break;
                    }
                }

                if let Some(sink) = sink_.upgrade() {
                    (sink.close)();
                }

                #[cfg(target_os = "windows")]
                if let Some(guard) = thread_class_guard {
                    drop(guard)
                }
            })?;

        Ok(AudioSender {
            sink: Arc::downgrade(sink),
            chunk_count,
            unparker,
            buffer,
        })
    }
}

impl FrameArrived for AudioSender {
    type Frame = AudioFrame;

    fn sink(&mut self, frame: &Self::Frame) -> bool {
        let mut buffer = self.buffer.lock().unwrap();
        buffer.extend_from_slice(unsafe {
            std::slice::from_raw_parts(
                frame.data as *const _,
                frame.frames as usize * size_of::<i16>(),
            )
        });

        if buffer.len() >= self.chunk_count * 2 {
            self.unparker.unpark();
        }

        if let Some(sink) = self.sink.upgrade() {
            (sink.audio)(frame);
        }

        true
    }
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
    pub fn new(options: SenderDescriptor, sink: FrameSink) -> Result<Self> {
        log::info!("create sender");

        let mut capture_options = CaptureDescriptor::default();
        let adapter = StreamSenderAdapter::new(options.multicast);
        let sink = Arc::new(sink);

        if let Some((source, options)) = options.audio {
            capture_options.audio = Some(SourceCaptureDescriptor {
                arrived: AudioSender::new(
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
        (self.sink.close)()
    }
}
