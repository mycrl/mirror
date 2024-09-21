use crate::FrameSink;

use std::{sync::Arc, thread};

use anyhow::Result;
use codec::{AudioDecoder, VideoDecoder, VideoDecoderSettings, VideoDecoderType};
use transport::adapter::{StreamKind, StreamMultiReceiverAdapter, StreamReceiverAdapterExt};

#[cfg(target_os = "windows")]
use utils::win32::MediaThreadClass;

#[derive(Debug, Clone)]
pub struct ReceiverDescriptor {
    pub video: VideoDecoderType,
}

fn create_video_decoder(
    adapter: &Arc<StreamMultiReceiverAdapter>,
    sink: &Arc<FrameSink>,
    settings: VideoDecoderSettings,
) -> Result<()> {
    let sink_ = Arc::downgrade(sink);
    let adapter_ = Arc::downgrade(adapter);
    let mut codec = VideoDecoder::new(settings)?;

    thread::Builder::new()
        .name("VideoDecoderThread".to_string())
        .spawn(move || {
            #[cfg(target_os = "windows")]
            let thread_class_guard = MediaThreadClass::Playback.join().ok();

            'a: while let (Some(adapter), Some(sink)) = (adapter_.upgrade(), sink_.upgrade()) {
                if let Some((packet, _, timestamp)) = adapter.next(StreamKind::Video) {
                    if let Err(e) = codec.decode(&packet, timestamp) {
                        log::error!("video decode error={:?}", e);

                        break;
                    } else {
                        while let Some(frame) = codec.read() {
                            if !(sink.video)(frame) {
                                log::warn!("video sink return false!");

                                break 'a;
                            }
                        }
                    }
                } else {
                    log::warn!("video adapter next is none!");

                    break;
                }
            }

            log::warn!("video decoder thread is closed!");
            if let Some(sink) = sink_.upgrade() {
                (sink.close)()
            }

            #[cfg(target_os = "windows")]
            if let Some(guard) = thread_class_guard {
                drop(guard)
            }
        })?;

    Ok(())
}

fn create_audio_decoder(
    adapter: &Arc<StreamMultiReceiverAdapter>,
    sink: &Arc<FrameSink>,
) -> Result<()> {
    let sink_ = Arc::downgrade(sink);
    let adapter_ = Arc::downgrade(adapter);
    let mut codec = AudioDecoder::new()?;

    thread::Builder::new()
        .name("AudioDecoderThread".to_string())
        .spawn(move || {
            #[cfg(target_os = "windows")]
            let thread_class_guard = MediaThreadClass::ProAudio.join().ok();

            'a: while let (Some(adapter), Some(sink)) = (adapter_.upgrade(), sink_.upgrade()) {
                if let Some((packet, _, timestamp)) = adapter.next(StreamKind::Audio) {
                    if let Err(e) = codec.decode(&packet, timestamp) {
                        log::error!("audio decode error={:?}", e);

                        break;
                    } else {
                        while let Some(frame) = codec.read() {
                            if !(sink.audio)(frame) {
                                log::warn!("audio sink return false!");

                                break 'a;
                            }
                        }
                    }
                } else {
                    log::warn!("audio adapter next is none!");

                    break;
                }
            }

            log::warn!("audio decoder thread is closed!");
            if let Some(sink) = sink_.upgrade() {
                (sink.close)()
            }

            #[cfg(target_os = "windows")]
            if let Some(guard) = thread_class_guard {
                drop(guard)
            }
        })?;

    Ok(())
}

pub struct Receiver {
    pub(crate) adapter: Arc<StreamMultiReceiverAdapter>,
    sink: Arc<FrameSink>,
}

impl Receiver {
    /// Create a receiving end. The receiving end is much simpler to implement.
    /// You only need to decode the data in the queue and call it back to the
    /// sink.
    pub fn new(options: ReceiverDescriptor, sink: FrameSink) -> Result<Self> {
        log::info!("create receiver");

        let adapter = StreamMultiReceiverAdapter::new();
        let sink = Arc::new(sink);

        create_audio_decoder(&adapter, &sink)?;
        create_video_decoder(
            &adapter,
            &sink,
            VideoDecoderSettings {
                codec: options.video,
                #[cfg(target_os = "windows")]
                direct3d: crate::DIRECT_3D_DEVICE.read().unwrap().clone(),
            },
        )?;

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
