use std::{
    sync::{Arc, RwLock, Weak},
    thread,
};

use anyhow::{anyhow, Result};
use bytes::Bytes;
use codec::{VideoDecoder, VideoEncoder, VideoEncoderSettings};
use common::frame::VideoFrame;
use devices::{Device, DeviceManager, DeviceManagerOptions, VideoInfo, VideoSink};
use once_cell::sync::Lazy;
use transport::{
    adapter::{StreamBufferInfo, StreamKind, StreamReceiverAdapter, StreamSenderAdapter},
    Transport,
};

static OPTIONS: Lazy<RwLock<MirrorOptions>> = Lazy::new(|| Default::default());

#[derive(Debug, Clone)]
pub struct VideoOptions {
    pub encoder: String,
    pub decoder: String,
    pub max_b_frames: u8,
    pub frame_rate: u8,
    pub width: u32,
    pub height: u32,
    pub bit_rate: u64,
    pub key_frame_interval: u32,
}

impl Default for VideoOptions {
    fn default() -> Self {
        Self {
            encoder: "libx264".to_string(),
            decoder: "h264".to_string(),
            max_b_frames: 0,
            frame_rate: 30,
            width: 1280,
            height: 720,
            bit_rate: 500 * 1024 * 8,
            key_frame_interval: 10,
        }
    }
}

impl Into<VideoEncoderSettings> for VideoOptions {
    fn into(self) -> VideoEncoderSettings {
        VideoEncoderSettings {
            width: self.width,
            height: self.height,
            bit_rate: self.bit_rate,
            frame_rate: self.frame_rate,
            max_b_frames: self.max_b_frames,
            key_frame_interval: self.key_frame_interval,
            codec_name: self.encoder,
        }
    }
}

#[derive(Debug, Clone)]
pub struct MirrorOptions {
    pub video: VideoOptions,
    pub multicast: String,
    pub mtu: usize,
}

impl Default for MirrorOptions {
    fn default() -> Self {
        Self {
            multicast: "239.0.0.1".to_string(),
            video: Default::default(),
            mtu: 1500,
        }
    }
}

pub fn init(options: MirrorOptions) -> Result<()> {
    {
        let mut path = std::env::current_exe()?;
        path.pop();
        std::env::set_current_dir(path)?;
    }

    #[cfg(debug_assertions)]
    {
        simple_logger::init_with_level(log::Level::Debug)?;
    }

    *OPTIONS.write().unwrap() = options.clone();
    log::info!("mirror init: options={:?}", options);

    Ok(devices::init(DeviceManagerOptions {
        video: VideoInfo {
            width: options.video.width,
            height: options.video.height,
            fps: options.video.frame_rate,
        },
    })?)
}

pub fn quit() {
    devices::quit();

    log::info!("close mirror");
}

pub fn set_input_device(device: &Device) {
    DeviceManager::set_input(device);

    log::info!("set input to device manager: device={:?}", device.name());
}

struct SenderObserver<F> {
    video_encoder: VideoEncoder,
    adapter: Weak<StreamSenderAdapter>,
    callback: Option<F>,
}

impl<F> SenderObserver<F>
where
    F: Fn(&VideoFrame) -> bool + Send + 'static,
{
    fn new(adapter: &Arc<StreamSenderAdapter>, callback: Option<F>) -> anyhow::Result<Self> {
        let options = OPTIONS.read().unwrap();
        Ok(Self {
            callback,
            adapter: Arc::downgrade(adapter),
            video_encoder: VideoEncoder::new(&options.video.clone().try_into()?)
                .ok_or_else(|| anyhow!("Failed to create video encoder."))?,
        })
    }
}

impl<F> VideoSink for SenderObserver<F>
where
    F: Fn(&VideoFrame) -> bool + Send + 'static,
{
    fn sink(&self, frame: &VideoFrame) {
        self.callback.as_ref().map(|it| it(frame));

        if let Some(adapter) = self.adapter.upgrade().as_ref() {
            if self.video_encoder.encode(frame) {
                while let Some(packet) = self.video_encoder.read() {
                    adapter.send(
                        Bytes::copy_from_slice(packet.buffer),
                        StreamBufferInfo::Video(packet.flags, 0),
                    );
                }
            }
        }
    }
}

pub struct Mirror(Transport);

impl Mirror {
    pub fn new() -> Result<Self> {
        let options = OPTIONS.read().unwrap();
        Ok(Self(Transport::new::<()>(
            options.mtu,
            options.multicast.parse()?,
            None,
        )?))
    }

    pub fn create_sender<F>(
        &self,
        bind: &str,
        callback: Option<F>,
    ) -> Result<Arc<StreamSenderAdapter>>
    where
        F: Fn(&VideoFrame) -> bool + Send + 'static,
    {
        log::info!("create sender: bind={}", bind);

        let adapter = StreamSenderAdapter::new();
        self.0
            .create_sender(0, bind.parse()?, Vec::new(), &adapter)?;

        devices::set_video_sink(SenderObserver::new(&adapter, callback)?);
        Ok(adapter)
    }

    pub fn create_receiver<F>(&self, bind: &str, callback: F) -> Result<Arc<StreamReceiverAdapter>>
    where
        F: Fn(&VideoFrame) -> bool + Send + 'static,
    {
        log::info!("create receiver: bind={}", bind);

        let options = OPTIONS.read().unwrap();
        let adapter = StreamReceiverAdapter::new();
        self.0.create_receiver(bind.parse()?, &adapter)?;

        let adapter_ = Arc::downgrade(&adapter);
        let video_decoder = VideoDecoder::new(&options.video.decoder)
            .ok_or_else(|| anyhow!("Failed to create video decoder."))?;

        thread::spawn(move || {
            while let Some(adapter) = adapter_.upgrade().as_ref() {
                'a: while let Some((packet, kind, _)) = adapter.next() {
                    if kind == StreamKind::Video {
                        if !video_decoder.decode(&packet) {
                            break;
                        }

                        while let Some(frame) = video_decoder.read() {
                            if !callback(frame) {
                                break 'a;
                            }
                        }
                    }
                }
            }
        });

        Ok(adapter)
    }
}
