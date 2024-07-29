pub mod mirror;
pub mod sender;

use std::{
    ffi::{c_char, c_int, c_void},
    fmt::Debug,
    ptr::null_mut,
    sync::Arc,
};

use capture::{CaptureSettings, Device, DeviceKind, DeviceManager};
use common::{
    frame::{AudioFrame, VideoFrame},
    jump_current_exe_dir,
    strings::Strings,
};

use mirror::{AudioOptions, FrameSink, Mirror, MirrorOptions, VideoOptions};
use transport::adapter::{
    StreamMultiReceiverAdapter, StreamReceiverAdapterExt, StreamSenderAdapter,
};

#[cfg(target_os = "windows")]
use windows::Win32::System::Threading::{GetCurrentProcess, SetPriorityClass, HIGH_PRIORITY_CLASS};

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct RawVideoOptions {
    /// Video encoder settings, possible values are `h264_qsv`, `h264_nvenc`,
    /// `libx264` and so on.
    pub encoder: *const c_char,
    /// Video decoder settings, possible values are `h264_qsv`, `h264_cuvid`,
    /// `h264`, etc.
    pub decoder: *const c_char,
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

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct RawMirrorOptions {
    /// Video Codec Configuration.
    pub video: RawVideoOptions,
    /// Audio Codec Configuration.
    pub audio: RawAudioOptions,
    /// mirror server address.
    pub server: *const c_char,
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
            server: Strings::from(self.server).to_string()?,
            video: self.video.try_into()?,
            audio: self.audio.into(),
            mtu: self.mtu,
        })
    }
}

#[no_mangle]
extern "system" fn DllMain(
    _dll_module: u32,
    _call_reason: usize,
    _reserved: *const c_void,
) -> bool {
    if jump_current_exe_dir().is_err() {
        return false;
    }

    #[cfg(debug_assertions)]
    {
        if common::logger::init("mirror.log", log::LevelFilter::Info).is_err() {
            return false;
        }

        std::panic::set_hook(Box::new(|info| {
            log::error!("{:?}", info);
        }));
    }
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

    true
}

/// Automatically search for encoders, limited hardware, fallback to software
/// implementation if hardware acceleration unit is not found.
#[no_mangle]
pub extern "C" fn mirror_find_video_encoder() -> *const c_char {
    unsafe { codec::video::codec_find_video_encoder() }
}

/// Automatically search for decoders, limited hardware, fallback to software
/// implementation if hardware acceleration unit is not found.
#[no_mangle]
pub extern "C" fn mirror_find_video_decoder() -> *const c_char {
    unsafe { codec::video::codec_find_video_decoder() }
}

/// Initialize the environment, which must be initialized before using the SDK.
#[no_mangle]
pub extern "C" fn mirror_init(options: RawMirrorOptions) -> bool {
    log::info!("extern api: mirror init");

    checker((|| mirror::init(options.try_into()?))()).is_ok()
}

/// Cleans up the environment when the SDK exits, and is recommended to be
/// called when the application exits.
#[no_mangle]
pub extern "C" fn mirror_quit() {
    log::info!("extern api: mirror quit");

    mirror::quit()
}

/// Get device name.
#[no_mangle]
pub extern "C" fn mirror_get_device_name(device: *const Device) -> *const c_char {
    assert!(!device.is_null());

    log::info!("extern api: mirror get device name");

    unsafe { &*device }.c_name()
}

/// Get device kind.
#[no_mangle]
pub extern "C" fn mirror_get_device_kind(device: *const Device) -> DeviceKind {
    assert!(!device.is_null());

    log::info!("extern api: mirror get device kind");

    unsafe { &*device }.kind()
}

#[repr(C)]
pub struct RawDevices {
    pub list: *const Device,
    pub capacity: usize,
    pub size: usize,
}

/// Get devices from device manager.
#[no_mangle]
pub extern "C" fn mirror_get_devices(
    kind: DeviceKind,
    settings: *const CaptureSettings,
) -> RawDevices {
    log::info!("extern api: mirror get devices: kind={:?}", kind);

    let devices = match checker(DeviceManager::get_devices(
        kind,
        if !settings.is_null() {
            Some(unsafe { &*settings })
        } else {
            None
        },
    )) {
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
#[no_mangle]
pub extern "C" fn mirror_devices_destroy(devices: *const RawDevices) {
    assert!(!devices.is_null());

    log::info!("extern api: mirror devices destroy");

    let devices = unsafe { &*devices };
    drop(unsafe { Vec::from_raw_parts(devices.list as *mut Device, devices.size, devices.size) })
}

/// Setting up an input device, repeated settings for the same type of device
/// will overwrite the previous device.
#[no_mangle]
pub extern "C" fn mirror_set_input_device(
    device: *const Device,
    settings: *const CaptureSettings,
) -> bool {
    assert!(!device.is_null());

    log::info!("extern api: mirror set input device");

    checker(mirror::set_input_device(
        unsafe { &*device },
        if !settings.is_null() {
            Some(unsafe { &*settings })
        } else {
            None
        },
    ))
    .is_ok()
}

/// Start capturing audio and video data.
#[no_mangle]
pub extern "C" fn mirror_start_capture() -> c_int {
    log::info!("extern api: mirror start capture devices");

    capture::start()
}

/// Stop capturing audio and video data.
#[no_mangle]
pub extern "C" fn mirror_stop_capture() {
    log::info!("extern api: mirror stop capture devices");

    capture::stop();
}

#[repr(C)]
pub struct RawMirror {
    mirror: Mirror,
}

/// Create mirror.
#[no_mangle]
pub extern "C" fn mirror_create() -> *const RawMirror {
    log::info!("extern api: mirror create");

    checker(Mirror::new())
        .map(|mirror| Box::into_raw(Box::new(RawMirror { mirror })))
        .unwrap_or_else(|_| null_mut()) as *const _
}

/// Release mirror.
#[no_mangle]
pub extern "C" fn mirror_destroy(mirror: *const RawMirror) {
    assert!(!mirror.is_null());

    log::info!("extern api: mirror destroy");

    capture::set_frame_sink::<()>(None);
    drop(unsafe { Box::from_raw(mirror as *mut RawMirror) });
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct RawFrameSink {
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
    pub video: Option<extern "C" fn(ctx: usize, frame: *const VideoFrame) -> bool>,
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
    pub audio: Option<extern "C" fn(ctx: usize, frame: *const AudioFrame) -> bool>,
    /// Callback when the sender is closed. This may be because the external
    /// side actively calls the close, or the audio and video packets cannot be
    /// sent (the network is disconnected), etc.
    pub close: Option<extern "C" fn(ctx: usize)>,
    pub ctx: usize,
}

impl Into<FrameSink> for RawFrameSink {
    fn into(self) -> FrameSink {
        FrameSink {
            video: Box::new(move |frame: &VideoFrame| {
                if let Some(callback) = &self.video {
                    callback(self.ctx, frame)
                } else {
                    true
                }
            }),
            audio: Box::new(move |frame: &AudioFrame| {
                if let Some(callback) = &self.audio {
                    callback(self.ctx, frame)
                } else {
                    true
                }
            }),
            close: Box::new(move || {
                log::info!("extern api: call close callback");

                if let Some(callback) = &self.close {
                    callback(self.ctx);

                    log::info!("extern api: call close callback done");
                }
            }),
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
#[no_mangle]
pub extern "C" fn mirror_create_sender(
    mirror: *const RawMirror,
    id: c_int,
    sink: RawFrameSink,
) -> *const RawSender {
    assert!(!mirror.is_null());

    log::info!("extern api: mirror create sender");

    checker((|| {
        unsafe { &*mirror }
            .mirror
            .create_sender(id as u32, sink.into())
    })())
    .map(|adapter| Box::into_raw(Box::new(RawSender { adapter })))
    .unwrap_or_else(|_| null_mut())
}

/// Set whether the sender uses multicast transmission.
#[no_mangle]
pub extern "C" fn mirror_sender_set_multicast(sender: *const RawSender, is_multicast: bool) {
    assert!(!sender.is_null());

    log::info!("extern api: mirror set sender multicast={}", is_multicast);

    unsafe { &*sender }.adapter.set_multicast(is_multicast);
}

/// Get whether the sender uses multicast transmission.
#[no_mangle]
pub extern "C" fn mirror_sender_get_multicast(sender: *const RawSender) -> bool {
    assert!(!sender.is_null());

    log::info!("extern api: mirror get sender multicast");

    unsafe { &*sender }.adapter.get_multicast()
}

/// Close sender.
#[no_mangle]
pub extern "C" fn mirror_sender_destroy(sender: *const RawSender) {
    assert!(!sender.is_null());

    log::info!("extern api: mirror close sender");

    capture::set_frame_sink::<()>(None);
    unsafe { Box::from_raw(sender as *mut RawSender) }
        .adapter
        .close();
}

#[repr(C)]
pub struct RawReceiver {
    adapter: Arc<StreamMultiReceiverAdapter>,
}

/// Create a receiver, specify a bound NIC address, you can pass callback to
/// get the sender's screen or sound callback, callback can not be null.
#[no_mangle]
pub extern "C" fn mirror_create_receiver(
    mirror: *const RawMirror,
    id: c_int,
    sink: RawFrameSink,
) -> *const RawReceiver {
    assert!(!mirror.is_null());

    log::info!("extern api: mirror create receiver");

    checker((|| {
        unsafe { &*mirror }
            .mirror
            .create_receiver(id as u32, sink.into())
    })())
    .map(|adapter| Box::into_raw(Box::new(RawReceiver { adapter })))
    .unwrap_or_else(|_| null_mut())
}

/// Close receiver.
#[no_mangle]
pub extern "C" fn mirror_receiver_destroy(receiver: *const RawReceiver) {
    assert!(!receiver.is_null());

    log::info!("extern api: mirror close receiver");

    unsafe { Box::from_raw(receiver as *mut RawReceiver) }
        .adapter
        .close();
}

#[inline]
fn checker<T, E: Debug>(result: Result<T, E>) -> Result<T, E> {
    if let Err(e) = &result {
        log::error!("{:?}", e);
    }

    result
}
