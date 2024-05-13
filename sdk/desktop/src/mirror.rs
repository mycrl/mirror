use std::{
    sync::{Arc, RwLock, Weak},
    thread,
};

#[cfg(feature = "audio")]
use codec::AudioDecoder;

use anyhow::Result;
use bytes::Bytes;
use capture::{AVFrameSink, AudioInfo, Device, DeviceManager, DeviceManagerOptions, VideoInfo};
use codec::{AudioEncoder, AudioEncoderSettings, VideoDecoder, VideoEncoder, VideoEncoderSettings};
use common::frame::{AudioFrame, VideoFrame};
use once_cell::sync::Lazy;
use transport::{
    adapter::{StreamBufferInfo, StreamKind, StreamReceiverAdapter, StreamSenderAdapter},
    Transport,
};

static OPTIONS: Lazy<RwLock<MirrorOptions>> = Lazy::new(Default::default);

/// Audio Codec Configuration.
#[derive(Debug, Clone)]
pub struct AudioOptions {
    /// Video encoder settings, possible values are `libopus`and so on.
    pub encoder: String,
    /// Video decoder settings, possible values are `libopus`and so on.
    pub decoder: String,
    /// The sample rate of the audio, in seconds.
    pub sample_rate: u64,
    /// The bit rate of the video encoding.
    pub bit_rate: u64,
}

impl Default for AudioOptions {
    fn default() -> Self {
        Self {
            encoder: "libopus".to_string(),
            decoder: "libopus".to_string(),
            sample_rate: 48000,
            bit_rate: 64000,
        }
    }
}

impl From<AudioOptions> for AudioEncoderSettings {
    fn from(val: AudioOptions) -> Self {
        AudioEncoderSettings {
            codec_name: val.encoder,
            bit_rate: val.bit_rate,
            sample_rate: val.sample_rate,
        }
    }
}

/// Video Codec Configuration.
#[derive(Debug, Clone)]
pub struct VideoOptions {
    /// Video encoder settings, possible values are `h264_qsv`, `h264_nvenc`,
    /// `libx264` and so on.
    pub encoder: String,
    /// Video decoder settings, possible values are `h264_qsv`, `h264_cuvid`,
    /// `h264`, etc.
    pub decoder: String,
    /// Maximum number of B-frames, if low latency encoding is performed, it is
    /// recommended to set it to 0 to indicate that no B-frames are encoded.
    pub max_b_frames: u8,
    /// Frame rate setting in seconds.
    pub frame_rate: u8,
    /// The width of the video.
    pub width: u32,
    /// The height of the video.
    pub height: u32,
    /// The bit rate of the video encoding.
    pub bit_rate: u64,
    /// Keyframe Interval, used to specify how many frames apart to output a
    /// keyframe.
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

impl From<VideoOptions> for VideoEncoderSettings {
    fn from(val: VideoOptions) -> Self {
        VideoEncoderSettings {
            width: val.width,
            height: val.height,
            bit_rate: val.bit_rate,
            frame_rate: val.frame_rate,
            max_b_frames: val.max_b_frames,
            key_frame_interval: val.key_frame_interval,
            codec_name: val.encoder,
        }
    }
}

#[derive(Debug, Clone)]
pub struct MirrorOptions {
    /// Video Codec Configuration.
    pub video: VideoOptions,
    /// Audio Codec Configuration.
    pub audio: AudioOptions,
    /// Multicast address, e.g. `239.0.0.1`.
    pub multicast: String,
    /// The size of the maximum transmission unit of the network, which is
    /// related to the settings of network devices such as routers or switches,
    /// the recommended value is 1400.
    pub mtu: usize,
}

impl Default for MirrorOptions {
    fn default() -> Self {
        Self {
            multicast: "239.0.0.1".to_string(),
            video: Default::default(),
            audio: Default::default(),
            mtu: 1500,
        }
    }
}

/// Initialize the environment, which must be initialized before using the SDK.
pub fn init(options: MirrorOptions) -> Result<()> {
    // Because of the path issues with OBS looking for plugins as well as data, the
    // working directory has to be adjusted to the directory where the current
    // executable is located.
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

    Ok(capture::init(DeviceManagerOptions {
        video: VideoInfo {
            width: options.video.width,
            height: options.video.height,
            fps: options.video.frame_rate,
        },
        audio: AudioInfo {
            samples_per_sec: options.audio.sample_rate as u32,
        },
    })?)
}

/// Cleans up the environment when the SDK exits, and is recommended to be
/// called when the application exits.
pub fn quit() {
    capture::quit();

    log::info!("close mirror");
}

/// Setting up an input device, repeated settings for the same type of device
/// will overwrite the previous device.
pub fn set_input_device(device: &Device) -> Result<()> {
    DeviceManager::set_input(device)?;

    log::info!("set input to device manager: device={:?}", device.name());
    Ok(())
}

pub struct FrameSink<A, V> {
    pub video: V,
    pub audio: A,
}

struct SenderObserver<A, V> {
    audio_encoder: AudioEncoder,
    video_encoder: VideoEncoder,
    adapter: Weak<StreamSenderAdapter>,
    sink: FrameSink<A, V>,
}

impl<A, V> SenderObserver<A, V>
where
    A: Fn(&AudioFrame) -> bool + Send + 'static,
    V: Fn(&VideoFrame) -> bool + Send + 'static,
{
    fn new(adapter: &Arc<StreamSenderAdapter>, sink: FrameSink<A, V>) -> anyhow::Result<Self> {
        let options = OPTIONS.read().unwrap();
        Ok(Self {
            sink,
            adapter: Arc::downgrade(adapter),
            video_encoder: VideoEncoder::new(&options.video.clone().into())?,
            audio_encoder: AudioEncoder::new(&options.audio.clone().into())?,
        })
    }
}

impl<A, V> AVFrameSink for SenderObserver<A, V>
where
    A: Fn(&AudioFrame) -> bool + Send + 'static,
    V: Fn(&VideoFrame) -> bool + Send + 'static,
{
    fn video(&self, frame: &VideoFrame) {
        (self.sink.video)(frame);

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

    fn audio(&self, frame: &AudioFrame) {
        (self.sink.audio)(frame);

        if self.audio_encoder.encode(frame) {
            if let Some(adapter) = self.adapter.upgrade().as_ref() {
                if self.audio_encoder.encode(frame) {
                    while let Some(packet) = self.audio_encoder.read() {
                        adapter.send(
                            Bytes::copy_from_slice(packet.buffer),
                            StreamBufferInfo::Audio(packet.flags, 0),
                        );
                    }
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

    /// Create a sender, specify a bound NIC address, you can pass callback to
    /// get the device screen or sound callback, callback can be null, if it is
    /// null then it means no callback data is needed.
    pub fn create_sender<A, V>(
        &self,
        bind: &str,
        sink: FrameSink<A, V>,
    ) -> Result<Arc<StreamSenderAdapter>>
    where
        A: Fn(&AudioFrame) -> bool + Send + 'static,
        V: Fn(&VideoFrame) -> bool + Send + 'static,
    {
        log::info!("create sender: bind={}", bind);

        let adapter = StreamSenderAdapter::new();
        self.0
            .create_sender(0, bind.parse()?, Vec::new(), &adapter)?;

        capture::set_frame_sink(SenderObserver::new(&adapter, sink)?);
        Ok(adapter)
    }

    /// Create a receiver, specify a bound NIC address, you can pass callback to
    /// get the sender's screen or sound callback, callback can not be null.
    pub fn create_receiver<A, V>(
        &self,
        bind: &str,
        sink: FrameSink<A, V>,
    ) -> Result<Arc<StreamReceiverAdapter>>
    where
        A: Fn(&AudioFrame) -> bool + Send + 'static,
        V: Fn(&VideoFrame) -> bool + Send + 'static,
    {
        log::info!("create receiver: bind={}", bind);

        let options = OPTIONS.read().unwrap();
        let adapter = StreamReceiverAdapter::new();
        self.0.create_receiver(bind.parse()?, &adapter)?;

        let video_decoder = VideoDecoder::new(&options.video.decoder)?;

        #[cfg(feature = "audio")]
        let audio_decoder = AudioDecoder::new(&options.audio.decoder)?;

        let adapter_ = adapter.clone();
        thread::spawn(move || {
            'a: while let Some((packet, kind, _)) = adapter_.next() {
                if packet.is_empty() {
                    continue;
                }

                if kind == StreamKind::Video {
                    if video_decoder.decode(&packet) {
                        while let Some(frame) = video_decoder.read() {
                            if !(sink.video)(frame) {
                                break 'a;
                            }
                        }
                    } else {
                        break;
                    }
                } else if kind == StreamKind::Audio {
                    #[cfg(feature = "audio")]
                    if audio_decoder.decode(&packet) {
                        while let Some(frame) = audio_decoder.read() {
                            if !(sink.audio)(frame) {
                                break 'a;
                            }
                        }
                    } else {
                        break;
                    }
                }
            }
        });

        Ok(adapter)
    }
}
