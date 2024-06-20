use std::{sync::Arc, thread};

use bytes::Bytes;
use capture::AVFrameSink;
use codec::{AudioEncoder, AudioEncoderSettings, VideoEncoder, VideoEncoderSettings};

use common::frame::{AudioFrame, VideoFrame};
use crossbeam::sync::{Parker, Unparker};
use transport::adapter::{StreamBufferInfo, StreamSenderAdapter};

use crate::mirror::{FrameSink, OPTIONS};

struct VideoSender {
    encoder: VideoEncoder,
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
    ) -> anyhow::Result<Arc<Self>> {
        let parker = Parker::new();
        let sender = Arc::new(VideoSender {
            encoder: VideoEncoder::new(settings)?,
            unparker: parker.unparker().clone(),
        });

        let sender_ = sender.clone();
        let adapter_ = Arc::downgrade(adapter);
        thread::spawn(move || {
            while let Some(adapter) = adapter_.upgrade() {
                // Waiting for external audio and video frame updates.
                parker.park();

                // Push the audio and video frames into the encoder.
                if sender_.encoder.encode() {
                    // Try to get the encoded data packets. The audio and video frames do not
                    // correspond to the data packets one by one, so you need to try to get multiple
                    // packets until they are empty.
                    while let Some(packet) = sender_.encoder.read() {
                        adapter.send(
                            Bytes::copy_from_slice(packet.buffer),
                            StreamBufferInfo::Video(packet.flags, 0),
                        );
                    }
                }
            }
        });

        Ok(sender)
    }

    // Copy the audio and video frames to the encoder and notify the encoding
    // thread.
    fn sink(&self, frame: &VideoFrame) {
        if self.encoder.send_frame(frame) {
            self.unparker.unpark();
        }
    }
}

struct AudioSender {
    encoder: AudioEncoder,
    unparker: Unparker,
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
        let parker = Parker::new();
        let sender = Arc::new(AudioSender {
            encoder: AudioEncoder::new(settings)?,
            unparker: parker.unparker().clone(),
        });

        let sender_ = sender.clone();
        let adapter_ = Arc::downgrade(adapter);
        thread::spawn(move || {
            while let Some(adapter) = adapter_.upgrade() {
                // Waiting for external audio and video frame updates.
                parker.park();

                // Push the audio and video frames into the encoder.
                if sender_.encoder.encode() {
                    // Try to get the encoded data packets. The audio and video frames do not
                    // correspond to the data packets one by one, so you need to try to get multiple
                    // packets until they are empty.
                    while let Some(packet) = sender_.encoder.read() {
                        adapter.send(
                            Bytes::copy_from_slice(packet.buffer),
                            StreamBufferInfo::Audio(packet.flags, 0),
                        );
                    }
                }
            }
        });

        Ok(sender)
    }

    // Copy the audio and video frames to the encoder and notify the encoding
    // thread.
    fn sink(&self, frame: &AudioFrame) {
        if self.encoder.send_frame(frame) {
            self.unparker.unpark();
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
