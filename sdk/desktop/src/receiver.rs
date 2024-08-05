use std::{sync::Arc, thread};

use anyhow::Result;
use codec::{AudioDecoder, VideoDecoder};
use transport::adapter::{StreamKind, StreamMultiReceiverAdapter, StreamReceiverAdapterExt};

use crate::factory::FrameSink;

#[derive(Debug, Clone)]
pub struct ReceiverOptions {
    pub video: String,
    pub audio: String,
}

fn create_video_decoder(
    adapter: &Arc<StreamMultiReceiverAdapter>,
    sink: &Arc<FrameSink>,
    codec: &str,
) -> Result<()> {
    let sink_ = Arc::downgrade(sink);
    let adapter_ = Arc::downgrade(adapter);
    let mut codec = VideoDecoder::new(codec)?;

    thread::Builder::new()
        .name("VideoDecoderThread".to_string())
        .spawn(move || {
            'a: while let (Some(adapter), Some(sink)) = (adapter_.upgrade(), sink_.upgrade()) {
                if let Some((packet, flags, timestamp)) = adapter.next(StreamKind::Video) {
                    if codec.decode(&packet, flags, timestamp) {
                        while let Some(frame) = codec.read() {
                            if !(sink.video)(frame) {
                                break 'a;
                            }
                        }
                    } else {
                        break;
                    }
                } else {
                    break;
                }
            }

            log::warn!("video decoder thread is closed!");
            if let Some(sink) = sink_.upgrade() {
                (sink.close)()
            }
        })?;

    Ok(())
}

fn create_audio_decoder(
    adapter: &Arc<StreamMultiReceiverAdapter>,
    sink: &Arc<FrameSink>,
    codec: &str,
) -> Result<()> {
    let sink_ = Arc::downgrade(sink);
    let adapter_ = Arc::downgrade(adapter);
    let mut codec = AudioDecoder::new(codec)?;

    thread::Builder::new()
        .name("AudioDecoderThread".to_string())
        .spawn(move || {
            'a: while let (Some(adapter), Some(sink)) = (adapter_.upgrade(), sink_.upgrade()) {
                if let Some((packet, flags, timestamp)) = adapter.next(StreamKind::Audio) {
                    if codec.decode(&packet, flags, timestamp) {
                        while let Some(frame) = codec.read() {
                            if !(sink.audio)(frame) {
                                break 'a;
                            }
                        }
                    } else {
                        break;
                    }
                } else {
                    break;
                }
            }

            log::warn!("audio decoder thread is closed!");
            if let Some(sink) = sink_.upgrade() {
                (sink.close)()
            }
        })?;

    Ok(())
}

pub struct Receiver {
    pub(crate) adapter: Arc<StreamMultiReceiverAdapter>,
    sink: Arc<FrameSink>,
}

impl Receiver {
    pub fn new(options: ReceiverOptions, sink: FrameSink) -> Result<Self> {
        log::info!("create receiver");

        let adapter = StreamMultiReceiverAdapter::new();
        let sink = Arc::new(sink);

        create_video_decoder(&adapter, &sink, &options.video)?;
        create_audio_decoder(&adapter, &sink, &options.audio)?;
        Ok(Self { adapter, sink })
    }
}

impl Drop for Receiver {
    fn drop(&mut self) {
        log::info!("receiver drop");

        self.adapter.close();
        (self.sink.close)()
    }
}
