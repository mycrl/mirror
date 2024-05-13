#![cfg(any(target_os = "windows", target_os = "linux"))]

pub mod mirror;

use std::{
    ffi::{c_char, c_void},
    fmt::Debug,
    ptr::null_mut,
    sync::Arc,
};

use capture::{Device, DeviceKind, DeviceManager};
use common::{
    frame::{AudioFrame, VideoFrame},
    strings::Strings,
};
use mirror::{AudioOptions, Mirror, MirrorOptions, VideoOptions};
use transport::adapter::{StreamReceiverAdapter, StreamSenderAdapter};

use crate::mirror::FrameSink;

/// Video Codec Configuration.
///
/// ```c
/// struct VideoOptions
/// {
///     char* encoder;
///     char* decoder;
///     uint8_t max_b_frames;
///     uint8_t frame_rate;
///     uint32_t width;
///     uint32_t height;
///     uint64_t bit_rate;
///     uint32_t key_frame_interval;
/// };
/// ```
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct RawVideoOptions {
    /// Video encoder settings, possible values are `h264_qsv`, `h264_nvenc`,
    /// `libx264` and so on.
    pub encoder: *const c_char,
    /// Video decoder settings, possible values are `h264_qsv`, `h264_cuvid`,
    /// `h264`, etc.
    pub decoder: *const c_char,
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

unsafe impl Send for RawVideoOptions {}
unsafe impl Sync for RawVideoOptions {}

impl TryInto<VideoOptions> for RawVideoOptions {
    type Error = anyhow::Error;

    fn try_into(self) -> Result<VideoOptions, Self::Error> {
        Ok(VideoOptions {
            encoder: Strings::from(self.encoder).to_string()?,
            decoder: Strings::from(self.decoder).to_string()?,
            key_frame_interval: self.key_frame_interval,
            max_b_frames: self.max_b_frames,
            frame_rate: self.frame_rate,
            width: self.width,
            height: self.height,
            bit_rate: self.bit_rate,
        })
    }
}

/// Audio Codec Configuration.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct RawAudioOptions {
    /// The sample rate of the audio, in seconds.
    pub sample_rate: u64,
    /// The bit rate of the video encoding.
    pub bit_rate: u64,
}

unsafe impl Send for RawAudioOptions {}
unsafe impl Sync for RawAudioOptions {}

impl Into<AudioOptions> for RawAudioOptions {
    fn into(self) -> AudioOptions {
        AudioOptions {
            encoder: "libopus".to_string(),
            decoder: "libopus".to_string(),
            sample_rate: self.sample_rate,
            bit_rate: self.bit_rate,
        }
    }
}

/// ```c
/// struct MirrorOptions
/// {
///     VideoOptions video;
///     char* multicast;
///     size_t mtu;
/// };
/// ```
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct RawMirrorOptions {
    /// Video Codec Configuration.
    pub video: RawVideoOptions,
    /// Audio Codec Configuration.
    pub audio: RawAudioOptions,
    /// Multicast address, e.g. `239.0.0.1`.
    pub multicast: *const c_char,
    /// The size of the maximum transmission unit of the network, which is
    /// related to the settings of network devices such as routers or switches,
    /// the recommended value is 1400.
    pub mtu: usize,
}

unsafe impl Send for RawMirrorOptions {}
unsafe impl Sync for RawMirrorOptions {}

impl TryInto<MirrorOptions> for RawMirrorOptions {
    type Error = anyhow::Error;

    fn try_into(self) -> Result<MirrorOptions, Self::Error> {
        Ok(MirrorOptions {
            multicast: Strings::from(self.multicast).to_string()?,
            video: self.video.try_into()?,
            audio: self.audio.into(),
            mtu: self.mtu,
        })
    }
}

/// Automatically search for encoders, limited hardware, fallback to software
/// implementation if hardware acceleration unit is not found.
///
/// ```c
/// EXPORT const char* mirror_find_video_encoder();
/// ```
#[no_mangle]
pub extern "C" fn mirror_find_video_encoder() -> *const c_char {
    unsafe { codec::video::codec_find_video_encoder() }
}

/// Automatically search for decoders, limited hardware, fallback to software
/// implementation if hardware acceleration unit is not found.
///
/// ```c
/// EXPORT const char* mirror_find_video_decoder();
/// ```
#[no_mangle]
pub extern "C" fn mirror_find_video_decoder() -> *const c_char {
    unsafe { codec::video::codec_find_video_decoder() }
}

/// Initialize the environment, which must be initialized before using the SDK.
///
/// ```c
/// EXPORT bool mirror_init(struct MirrorOptions options);
/// ```
#[no_mangle]
pub extern "C" fn mirror_init(options: RawMirrorOptions) -> bool {
    checker((|| mirror::init(options.try_into()?))()).is_ok()
}

/// Cleans up the environment when the SDK exits, and is recommended to be
/// called when the application exits.
///
/// ```c
/// EXPORT void mirror_quit();
/// ```
#[no_mangle]
pub extern "C" fn mirror_quit() {
    mirror::quit()
}

/// Get device name.
///
/// ```c
/// EXPORT const char* mirror_get_device_name(const struct Device* device);
/// ```
#[no_mangle]
pub extern "C" fn mirror_get_device_name(device: *const Device) -> *const c_char {
    assert!(!device.is_null());

    unsafe { &*device }.c_name()
}

/// Get device kind.
///
/// ```c
/// EXPORT enum DeviceKind mirror_get_device_kind(const struct Device* device);
/// ```
#[no_mangle]
pub extern "C" fn mirror_get_device_kind(device: *const Device) -> DeviceKind {
    assert!(!device.is_null());

    unsafe { &*device }.kind()
}

#[repr(C)]
pub struct RawDevices {
    pub list: *const Device,
    pub capacity: usize,
    pub size: usize,
}

/// Get devices from device manager.
///
/// ```c
/// EXPORT struct Devices mirror_get_devices(enum DeviceKind kind);
/// ```
#[no_mangle]
pub extern "C" fn mirror_get_devices(kind: DeviceKind) -> RawDevices {
    log::info!("get devices: kind={:?}", kind);

    let devices = match checker(DeviceManager::get_devices(kind)) {
        Ok(it) => it.to_vec(),
        Err(_) => Vec::new(),
    };

    let raw_devices = RawDevices {
        capacity: devices.capacity(),
        list: devices.as_ptr(),
        size: devices.len(),
    };

    #[cfg(debug_assertions)]
    {
        for device in &devices {
            log::info!("Device: name={:?}", device.name());
        }
    }

    std::mem::forget(devices);
    raw_devices
}

/// Release devices.
///
/// ```c
/// EXPORT void mirror_drop_devices(struct Devices* devices);
/// ```
#[no_mangle]
pub extern "C" fn mirror_drop_devices(devices: *const RawDevices) {
    assert!(!devices.is_null());

    let devices = unsafe { &*devices };
    drop(unsafe { Vec::from_raw_parts(devices.list as *mut Device, devices.size, devices.size) })
}

/// Setting up an input device, repeated settings for the same type of device
/// will overwrite the previous device.
///
/// ```c
/// EXPORT void mirror_set_input_device(const struct Device* device);
/// ```
#[no_mangle]
pub extern "C" fn mirror_set_input_device(device: *const Device) -> bool {
    assert!(!device.is_null());

    checker(mirror::set_input_device(unsafe { &*device })).is_ok()
}

#[repr(C)]
pub struct RawMirror {
    mirror: Mirror,
}

/// Create mirror.
///
/// ```c
/// EXPORT Mirror mirror_create();
/// ```
#[no_mangle]
pub extern "C" fn mirror_create() -> *const RawMirror {
    checker(Mirror::new())
        .map(|mirror| Box::into_raw(Box::new(RawMirror { mirror })))
        .unwrap_or_else(|_| null_mut()) as *const _
}

/// Release mirror.
///
/// ```c
/// EXPORT void mirror_drop(Mirror mirror);
/// ```
#[no_mangle]
pub extern "C" fn mirror_drop(mirror: *const RawMirror) {
    assert!(!mirror.is_null());

    drop(unsafe { Box::from_raw(mirror as *mut RawMirror) });

    log::info!("close mirror");
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct RawFrameSink {
    pub video: Option<extern "C" fn(ctx: *const c_void, frame: *const VideoFrame) -> bool>,
    pub audio: Option<extern "C" fn(ctx: *const c_void, frame: *const AudioFrame) -> bool>,
    pub ctx: *const c_void,
}

unsafe impl Send for RawFrameSink {}
unsafe impl Sync for RawFrameSink {}

impl RawFrameSink {
    fn send_video(&self, frame: &VideoFrame) -> bool {
        if let Some(callback) = &self.video {
            callback(self.ctx, frame)
        } else {
            true
        }
    }

    fn send_audio(&self, frame: &AudioFrame) -> bool {
        if let Some(callback) = &self.audio {
            callback(self.ctx, frame)
        } else {
            true
        }
    }
}

#[repr(C)]
pub struct RawSender {
    adapter: Arc<StreamSenderAdapter>,
}

/// Create a sender, specify a bound NIC address, you can pass callback to
/// get the device screen or sound callback, callback can be null, if it is
/// null then it means no callback data is needed.
///
/// ```c
/// EXPORT Sender mirror_create_sender(Mirror mirror, char* bind, ReceiverFrameCallback proc, void* ctx);
/// ```
#[no_mangle]
pub extern "C" fn mirror_create_sender(
    mirror: *const RawMirror,
    bind: *const c_char,
    sink: RawFrameSink,
) -> *const RawSender {
    assert!(!mirror.is_null());
    assert!(!bind.is_null());

    checker((|| {
        unsafe { &*mirror }.mirror.create_sender(
            &Strings::from(bind).to_string()?,
            FrameSink {
                video: move |frame: &VideoFrame| sink.send_video(frame),
                audio: move |frame: &AudioFrame| sink.send_audio(frame),
            },
        )
    })())
    .map(|adapter| Box::into_raw(Box::new(RawSender { adapter })))
    .unwrap_or_else(|_| null_mut())
}

/// Close sender.
///
/// ```c
/// EXPORT void mirror_close_sender(Sender sender);
/// ```
#[no_mangle]
pub extern "C" fn mirror_close_sender(sender: *const RawSender) {
    assert!(!sender.is_null());

    unsafe { Box::from_raw(sender as *mut RawSender) }
        .adapter
        .close();

    log::info!("close sender");
}

#[repr(C)]
pub struct RawReceiver {
    adapter: Arc<StreamReceiverAdapter>,
}

/// Create a receiver, specify a bound NIC address, you can pass callback to
/// get the sender's screen or sound callback, callback can not be null.
///
/// ```c
/// EXPORT Receiver mirror_create_receiver(Mirror mirror, char* bind, ReceiverFrameCallback proc, void* ctx);
/// ```
#[no_mangle]
pub extern "C" fn mirror_create_receiver(
    mirror: *const RawMirror,
    bind: *const c_char,
    sink: RawFrameSink,
) -> *const RawReceiver {
    assert!(!mirror.is_null());
    assert!(!bind.is_null());

    checker((|| {
        unsafe { &*mirror }.mirror.create_receiver(
            &Strings::from(bind).to_string()?,
            FrameSink {
                video: move |frame: &VideoFrame| sink.send_video(frame),
                audio: move |frame: &AudioFrame| sink.send_audio(frame),
            },
        )
    })())
    .map(|adapter| Box::into_raw(Box::new(RawReceiver { adapter })))
    .unwrap_or_else(|_| null_mut())
}

/// Close receiver.
///
/// ```c
/// EXPORT void mirror_close_receiver(Receiver receiver);
/// ```
#[no_mangle]
pub extern "C" fn mirror_close_receiver(receiver: *const RawReceiver) {
    assert!(!receiver.is_null());

    unsafe { Box::from_raw(receiver as *mut RawReceiver) }
        .adapter
        .close();

    log::info!("close receiver");
}

#[inline]
fn checker<T, E: Debug>(result: Result<T, E>) -> Result<T, E> {
    if let Err(e) = &result {
        log::error!("{:?}", e);
    }

    result
}
