use std::sync::{Arc, Mutex, Weak};

use bytes::BytesMut;
use capture::AVFrameSink;
use codec::{
    audio::create_opus_identification_header, AudioEncoder, AudioEncoderSettings, VideoEncoder,
    VideoEncoderSettings,
};

use common::frame::{AudioFormat, AudioFrame, VideoFrame};
use transport::{
    adapter::{BufferFlag, StreamBufferInfo, StreamSenderAdapter},
    package,
};

use crate::mirror::{FrameSink, OPTIONS};

struct VideoSender {
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
        adapter: &Arc<StreamSenderAdapter>,
        settings: &VideoEncoderSettings,
    ) -> anyhow::Result<Arc<Self>> {
        Ok(Arc::new(Self {
            encoder: VideoEncoder::new(settings)?,
            adapter: Arc::downgrade(adapter),
        }))
    }

    // Copy the audio and video frames to the encoder and notify the encoding
    // thread.
    fn sink(&self, frame: &VideoFrame) {
        if let Some(adapter) = self.adapter.upgrade() {
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
        }
    }
}

struct AudioSender {
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
    ) -> anyhow::Result<Arc<Self>> {
        Ok(Arc::new(AudioSender {
            encoder: AudioEncoder::new(settings)?,
            adapter: Arc::downgrade(adapter),
            buffer: Mutex::new(BytesMut::with_capacity(48000)),
            frames: settings.sample_rate as usize / 1000 * 100,
        }))
    }

    // Copy the audio and video frames to the encoder and notify the encoding
    // thread.
    fn sink(&self, frame: &AudioFrame) {
        if let Some(adapter) = self.adapter.upgrade() {
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
        }
    }
}

pub(crate) struct SenderObserver {
    video: Arc<VideoSender>,
    audio: Arc<AudioSender>,
    sink: FrameSink,
}

impl AVFrameSink for SenderObserver {
    fn video(&self, frame: &VideoFrame) {
        self.video.sink(frame);

        // Push the video frames to the external device, which can be used for rendering
        // to an external surface, etc.
        (self.sink.video)(frame);
    }

    fn audio(&self, frame: &AudioFrame) {
        self.audio.sink(frame);

        // Push the audio frame to the external device, which can then play it on the
        // speaker.
        (self.sink.audio)(frame);
    }
}

impl SenderObserver {
    pub(crate) fn new(adapter: &Arc<StreamSenderAdapter>, sink: FrameSink) -> anyhow::Result<Self> {
        let options = OPTIONS.read().unwrap();

        // Create an opus header data. The opus decoder needs this data to obtain audio
        // information. Here, actively add an opus header information to the queue, and
        // the transport layer will automatically cache it.
        adapter.send(
            package::copy_from_slice(&create_opus_identification_header(
                1,
                options.audio.sample_rate as u32,
            )),
            StreamBufferInfo::Audio(BufferFlag::Config as i32, 0),
        );

        Ok(Self {
            video: VideoSender::new(adapter, &options.video.clone().into())?,
            audio: AudioSender::new(adapter, &options.audio.clone().into())?,
            sink,
        })
    }
}

impl Drop for SenderObserver {
    fn drop(&mut self) {
        (self.sink.close)()
    }
}
