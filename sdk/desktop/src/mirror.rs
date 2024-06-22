use std::{
    sync::{Arc, RwLock},
    thread,
};

use crate::sender::SenderObserver;

use anyhow::Result;
use capture::{AudioInfo, Device, DeviceManager, DeviceManagerOptions, VideoInfo};
use codec::{AudioDecoder, AudioEncoderSettings, VideoDecoder, VideoEncoderSettings};
use common::{
    frame::{AudioFrame, VideoFrame},
    jump_current_exe_dir, logger,
};

use log::LevelFilter;
use once_cell::sync::Lazy;
use transport::{
    adapter::{StreamKind, StreamMultiReceiverAdapter, StreamSenderAdapter},
    Transport, TransportOptions,
};

#[cfg(target_os = "windows")]
use windows::Win32::System::Threading::{GetCurrentProcess, SetPriorityClass, HIGH_PRIORITY_CLASS};

pub static OPTIONS: Lazy<RwLock<MirrorOptions>> = Lazy::new(Default::default);

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
    /// mirror server address.
    pub server: String,
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
            server: "127.0.0.1".to_string(),
            video: Default::default(),
            audio: Default::default(),
            mtu: 1500,
        }
    }
}

/// Initialize the environment, which must be initialized before using the SDK.
pub fn init(options: MirrorOptions) -> Result<()> {
    jump_current_exe_dir()?;

    #[cfg(debug_assertions)]
    logger::init("mirror.log", LevelFilter::Info)?;

    // In order to prevent other programs from affecting the delay performance of
    // the current program, set the priority of the current process to high.
    #[cfg(target_os = "windows")]
    {
        if unsafe { SetPriorityClass(GetCurrentProcess(), HIGH_PRIORITY_CLASS) }.is_err() {
            log::error!(
                "failed to set current process priority, Maybe it's \
                because you didn't run it with administrator privileges."
            );
        }
    }

    *OPTIONS.write().unwrap() = options.clone();
    log::info!("mirror init: options={:?}", options);

    transport::init();
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
    transport::exit();

    log::info!("close mirror");
}

/// Setting up an input device, repeated settings for the same type of device
/// will overwrite the previous device.
pub fn set_input_device(device: &Device) -> Result<()> {
    DeviceManager::set_input(device)?;

    log::info!("set input to device manager: device={:?}", device.name());
    Ok(())
}

pub struct FrameSink {
    /// Callback occurs when the video frame is updated. The video frame format
    /// is fixed to NV12. Be careful not to call blocking methods inside the
    /// callback, which will seriously slow down the encoding and decoding
    /// pipeline.
    ///
    /// YCbCr (NV12)
    ///
    /// YCbCr, Y′CbCr, or Y Pb/Cb Pr/Cr, also written as YCBCR or Y′CBCR, is a
    /// family of color spaces used as a part of the color image pipeline in
    /// video and digital photography systems. Y′ is the luma component and
    /// CB and CR are the blue-difference and red-difference chroma
    /// components. Y′ (with prime) is distinguished from Y, which is
    /// luminance, meaning that light intensity is nonlinearly encoded based
    /// on gamma corrected RGB primaries.
    ///
    /// Y′CbCr color spaces are defined by a mathematical coordinate
    /// transformation from an associated RGB primaries and white point. If
    /// the underlying RGB color space is absolute, the Y′CbCr color space
    /// is an absolute color space as well; conversely, if the RGB space is
    /// ill-defined, so is Y′CbCr. The transformation is defined in
    /// equations 32, 33 in ITU-T H.273. Nevertheless that rule does not
    /// apply to P3-D65 primaries used by Netflix with BT.2020-NCL matrix,
    /// so that means matrix was not derived from primaries, but now Netflix
    /// allows BT.2020 primaries (since 2021). The same happens with
    /// JPEG: it has BT.601 matrix derived from System M primaries, yet the
    /// primaries of most images are BT.709.
    pub video: Box<dyn Fn(&VideoFrame) -> bool + Send + Sync>,
    /// Callback is called when the audio frame is updated. The audio frame
    /// format is fixed to PCM. Be careful not to call blocking methods inside
    /// the callback, which will seriously slow down the encoding and decoding
    /// pipeline.
    ///
    /// Pulse-code modulation
    ///
    /// Pulse-code modulation (PCM) is a method used to digitally represent
    /// analog signals. It is the standard form of digital audio in
    /// computers, compact discs, digital telephony and other digital audio
    /// applications. In a PCM stream, the amplitude of the analog signal is
    /// sampled at uniform intervals, and each sample is quantized to the
    /// nearest value within a range of digital steps.
    ///
    /// Linear pulse-code modulation (LPCM) is a specific type of PCM in which
    /// the quantization levels are linearly uniform. This is in contrast to
    /// PCM encodings in which quantization levels vary as a function of
    /// amplitude (as with the A-law algorithm or the μ-law algorithm).
    /// Though PCM is a more general term, it is often used to describe data
    /// encoded as LPCM.
    ///
    /// A PCM stream has two basic properties that determine the stream's
    /// fidelity to the original analog signal: the sampling rate, which is
    /// the number of times per second that samples are taken; and the bit
    /// depth, which determines the number of possible digital values that
    /// can be used to represent each sample.
    pub audio: Box<dyn Fn(&AudioFrame) -> bool + Send + Sync>,
    /// Callback when the sender is closed. This may be because the external
    /// side actively calls the close, or the audio and video packets cannot be
    /// sent (the network is disconnected), etc.
    pub close: Box<dyn Fn() + Send + Sync>,
}

pub struct Mirror(Transport);

impl Mirror {
    pub fn new() -> Result<Self> {
        let options = OPTIONS.read().unwrap();
        Ok(Self(Transport::new(TransportOptions {
            multicast: options.multicast.parse()?,
            server: options.server.parse()?,
            mtu: options.mtu,
        })?))
    }

    /// Create a sender, specify a bound NIC address, you can pass callback to
    /// get the device screen or sound callback, callback can be null, if it is
    /// null then it means no callback data is needed.
    pub fn create_sender(&self, id: u32, sink: FrameSink) -> Result<Arc<StreamSenderAdapter>> {
        log::info!("create sender: id={}", id);

        let adapter = StreamSenderAdapter::new();
        self.0.create_sender(id, &adapter)?;

        capture::set_frame_sink(Some(SenderObserver::new(&adapter, sink)?));
        Ok(adapter)
    }

    /// Create a receiver, specify a bound NIC address, you can pass callback to
    /// get the sender's screen or sound callback, callback can not be null.
    pub fn create_receiver(
        &self,
        id: u32,
        sink: FrameSink,
    ) -> Result<Arc<StreamMultiReceiverAdapter>> {
        log::info!("create receiver: id={}", id);

        let options = OPTIONS.read().unwrap();
        let adapter = StreamMultiReceiverAdapter::new();
        self.0.create_receiver(id, &adapter)?;

        let sink = Arc::new(sink);
        let video_decoder = VideoDecoder::new(&options.video.decoder)?;
        let audio_decoder = AudioDecoder::new(&options.audio.decoder)?;

        let sink_ = sink.clone();
        let adapter_ = adapter.clone();
        thread::spawn(move || {
            'a: while let Some((packet, _, _)) = adapter_.next(StreamKind::Video) {
                if video_decoder.decode(&packet) {
                    while let Some(frame) = video_decoder.read() {
                        if !(sink_.video)(frame) {
                            break 'a;
                        }
                    }
                } else {
                    break;
                }
            }

            (sink_.close)()
        });

        let adapter_ = adapter.clone();
        thread::spawn(move || {
            'a: while let Some((packet, _, _)) = adapter_.next(StreamKind::Audio) {
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

            log::warn!("audio decoder thread is closed!");
        });

        Ok(adapter)
    }
}
