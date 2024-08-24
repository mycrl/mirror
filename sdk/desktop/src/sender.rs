use crate::factory::FrameSink;

use std::{
    mem::size_of,
    sync::{Arc, Mutex, Weak},
    thread,
};

use anyhow::Result;
use bytes::BytesMut;
use capture::{
    AudioCaptureSourceDescription, Capture, CaptureOptions, FrameArrived, Size, Source,
    SourceCaptureOptions, VideoCaptureSourceDescription,
};

use codec::{
    audio::create_opus_identification_header, AudioEncoder, AudioEncoderSettings, VideoEncoder,
    VideoEncoderSettings,
};

use crossbeam::sync::{Parker, Unparker};
use frame::{AudioFrame, VideoFrame};
use transport::{
    adapter::{BufferFlag, StreamBufferInfo, StreamSenderAdapter},
    package,
};

#[cfg(target_os = "windows")]
use utils::win32::{create_d3d_device, MediaThreadClass};

struct VideoSender {
    encoder: Arc<Mutex<VideoEncoder>>,
    sink: Weak<FrameSink>,
    unparker: Unparker,
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
        let parker = Parker::new();
        let unparker = parker.unparker().clone();
        let encoder = Arc::new(Mutex::new(VideoEncoder::new(settings)?));

        let sink_ = Arc::downgrade(sink);
        let adapter_ = Arc::downgrade(adapter);
        let encoder_ = Arc::downgrade(&encoder);
        thread::Builder::new()
            .name("VideoEncoderThread".to_string())
            .spawn(move || {
                #[cfg(target_os = "windows")]
                let thread_class_guard = MediaThreadClass::DisplayPostProcessing.join().ok();

                loop {
                    parker.park();

                    if let (Some(adapter), Some(codec)) = (adapter_.upgrade(), encoder_.upgrade()) {
                        let mut encoder = codec.lock().unwrap();

                        // Try to get the encoded data packets. The audio and video frames do not
                        // correspond to the data packets one by one, so you need to try to get
                        // multiple packets until they are empty.
                        if encoder.encode() {
                            while let Some(packet) = encoder.read() {
                                adapter.send(
                                    package::copy_from_slice(packet.buffer),
                                    StreamBufferInfo::Video(packet.flags, packet.timestamp),
                                );
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

        Ok(Self {
            sink: Arc::downgrade(sink),
            unparker,
            encoder,
        })
    }
}

impl FrameArrived for VideoSender {
    type Frame = VideoFrame;

    fn sink(&mut self, frame: &Self::Frame) -> bool {
        // // Push the audio and video frames into the encoder.
        if self.encoder.lock().unwrap().send_frame(frame) {
            self.unparker.unpark();
        } else {
            return false;
        }

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

                        if encoder.send_frame(&frame) {
                            // Push the audio and video frames into the encoder.
                            if encoder.encode() {
                                // Try to get the encoded data packets. The audio and video frames
                                // do not correspond to the data
                                // packets one by one, so you need to try to get
                                // multiple packets until they are empty.
                                while let Some(packet) = encoder.read() {
                                    adapter.send(
                                        package::copy_from_slice(packet.buffer),
                                        StreamBufferInfo::Audio(packet.flags, packet.timestamp),
                                    );
                                }
                            } else {
                                break;
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

        #[cfg(target_os = "windows")]
        let direct3d = create_d3d_device()?;

        let mut capture_options = CaptureOptions::default();
        let adapter = StreamSenderAdapter::new(options.multicast);
        let sink = Arc::new(sink);

        if let Some((source, options)) = options.audio {
            capture_options.audio = Some(SourceCaptureOptions {
                arrived: AudioSender::new(&adapter, &options, &sink)?,
                description: AudioCaptureSourceDescription {
                    sample_rate: options.sample_rate as u32,
                    source,
                },
            });
        }

        if let Some((source, options)) = options.video {
            capture_options.video = Some(SourceCaptureOptions {
                arrived: VideoSender::new(&adapter, &options, &sink)?,
                description: VideoCaptureSourceDescription {
                    fps: options.frame_rate,
                    size: Size {
                        width: options.width,
                        height: options.height,
                    },
                    source,
                    #[cfg(target_os = "windows")]
                    direct3d,
                },
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
