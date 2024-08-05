mod factory;
mod receiver;

#[cfg(not(target_os = "macos"))]
mod sender;

use std::{
    ffi::{c_char, c_int},
    fmt::Debug,
    ptr::null_mut,
    sync::atomic::AtomicBool,
};

#[cfg(not(target_os = "macos"))]
use std::{ffi::CString, mem::ManuallyDrop};

use anyhow::ensure;
use common::{
    atomic::EasyAtomic,
    frame::{AudioFrame, VideoFrame},
    jump_current_exe_dir,
    strings::Strings,
};

#[cfg(target_os = "windows")]
use windows::Win32::System::Threading::{GetCurrentProcess, SetPriorityClass, HIGH_PRIORITY_CLASS};

#[no_mangle]
#[cfg(target_os = "windows")]
extern "system" fn DllMain(
    _dll_module: u32,
    _call_reason: usize,
    _reserved: *const std::ffi::c_void,
) -> bool {
    if !mirror_load() {
        return false;
    }

    // In order to prevent other programs from affecting the delay performance of
    // the current program, set the priority of the current process to high.
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

/// Because Linux does not have DllMain, you need to call it manually to achieve
/// similar behavior.
pub extern "C" fn mirror_load() -> bool {
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
pub extern "C" fn mirror_startup() -> bool {
    log::info!("extern api: mirror init");

    checker(factory::startup()).is_ok()
}

/// Cleans up the environment when the SDK exits, and is recommended to be
/// called when the application exits.
#[no_mangle]
pub extern "C" fn mirror_shutdown() {
    log::info!("extern api: mirror quit");

    let _ = checker(factory::shutdown());
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct MirrorOptions {
    pub server: *const c_char,
    pub multicast: *const c_char,
    pub mtu: usize,
}

impl TryInto<transport::TransportOptions> for MirrorOptions {
    type Error = anyhow::Error;

    fn try_into(self) -> Result<transport::TransportOptions, Self::Error> {
        Ok(transport::TransportOptions {
            multicast: Strings::from(self.multicast).to_string()?.parse()?,
            server: Strings::from(self.server).to_string()?.parse()?,
            mtu: self.mtu,
        })
    }
}

#[repr(C)]
pub struct Mirror(factory::Mirror);

/// Create mirror.
#[no_mangle]
pub extern "C" fn mirror_create(options: MirrorOptions) -> *const Mirror {
    log::info!("extern api: mirror create");

    let func = || factory::Mirror::new(options.try_into()?);

    checker(func())
        .map(|mirror| Box::into_raw(Box::new(Mirror(mirror))))
        .unwrap_or_else(|_| null_mut()) as *const _
}

/// Release mirror.
#[no_mangle]
pub extern "C" fn mirror_destroy(mirror: *const Mirror) {
    assert!(!mirror.is_null());

    log::info!("extern api: mirror destroy");
    drop(unsafe { Box::from_raw(mirror as *mut Mirror) });
}

#[repr(C)]
#[derive(Clone, Copy)]
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

impl Into<factory::FrameSink> for FrameSink {
    fn into(self) -> factory::FrameSink {
        let is_closed = AtomicBool::new(false);
        factory::FrameSink {
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
                if !is_closed.get() {
                    log::info!("extern api: call close callback");

                    if let Some(callback) = &self.close {
                        callback(self.ctx);
                        is_closed.update(true);

                        log::info!("extern api: call close callback done");
                    }
                }
            }),
        }
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct VideoOptions {
    pub codec: *const c_char,
    pub frame_rate: u8,
    pub width: u32,
    pub height: u32,
    pub bit_rate: u64,
    pub key_frame_interval: u32,
}

impl TryInto<codec::VideoEncoderSettings> for VideoOptions {
    type Error = anyhow::Error;

    fn try_into(self) -> Result<codec::VideoEncoderSettings, Self::Error> {
        Ok(codec::VideoEncoderSettings {
            codec: Strings::from(self.codec).to_string()?,
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
pub struct AudioOptions {
    pub sample_rate: u64,
    pub bit_rate: u64,
}

impl Into<codec::AudioEncoderSettings> for AudioOptions {
    fn into(self) -> codec::AudioEncoderSettings {
        codec::AudioEncoderSettings {
            codec: "libopus".to_string(),
            sample_rate: self.sample_rate,
            bit_rate: self.bit_rate,
        }
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct SenderOptions {
    video: VideoOptions,
    audio: AudioOptions,
    multicast: bool,
}

impl TryInto<sender::SenderOptions> for SenderOptions {
    type Error = anyhow::Error;

    fn try_into(self) -> Result<sender::SenderOptions, Self::Error> {
        Ok(sender::SenderOptions {
            audio: self.audio.try_into()?,
            video: self.video.try_into()?,
            multicast: self.multicast,
        })
    }
}

#[repr(C)]
#[cfg(not(target_os = "macos"))]
pub struct Sender(sender::Sender);

/// Create a sender, specify a bound NIC address, you can pass callback to
/// get the device screen or sound callback, callback can be null, if it is
/// null then it means no callback data is needed.
#[no_mangle]
#[rustfmt::skip]
#[cfg(not(target_os = "macos"))]
pub extern "C" fn mirror_create_sender(
    mirror: *const Mirror,
    id: c_int,
    options: SenderOptions,
    sink: FrameSink,
) -> *const Sender {
    assert!(!mirror.is_null());

    log::info!("extern api: mirror create sender");

    let func = || {
        let options: sender::SenderOptions = options.try_into()?;
        
        log::info!("mirror create options={:?}", options);
        
        ensure!(options.video.codec == "libx264" || options.video.codec == "h264_qsv", "invalid video encoder");
        ensure!(options.video.width % 4 == 0 && options.video.width <= 4096, "invalid video width");
        ensure!(options.video.height % 4 == 0 && options.video.height <= 2560, "invalid video height");
        ensure!(options.video.frame_rate <= 60, "invalid video frame rate");

        unsafe { &*mirror }
            .0
            .create_sender(id as u32, options, sink.into())
    };

    checker(func())
    .map(|sender| Box::into_raw(Box::new(Sender(sender))))
    .unwrap_or_else(|_| null_mut())
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceType {
    Camera,
    Screen,
    Audio,
}

impl Into<capture::SourceType> for SourceType {
    fn into(self) -> capture::SourceType {
        match self {
            Self::Camera => capture::SourceType::Camera,
            Self::Screen => capture::SourceType::Screen,
            Self::Audio => capture::SourceType::Audio,
        }
    }
}

impl From<capture::SourceType> for SourceType {
    fn from(value: capture::SourceType) -> Self {
        match value {
            capture::SourceType::Camera => Self::Camera,
            capture::SourceType::Screen => Self::Screen,
            capture::SourceType::Audio => Self::Audio,
        }
    }
}

#[repr(C)]
#[derive(Debug)]
#[cfg(not(target_os = "macos"))]
pub struct Source {
    index: usize,
    kind: SourceType,
    id: *const c_char,
    name: *const c_char,
}

#[repr(C)]
#[derive(Debug)]
#[cfg(not(target_os = "macos"))]
pub struct Sources {
    items: *mut Source,
    capacity: usize,
    size: usize,
}

/// Get capture sources from sender.
#[no_mangle]
#[cfg(not(target_os = "macos"))]
pub extern "C" fn mirror_sender_get_sources(sender: *const Sender, kind: SourceType) -> Sources {
    assert!(!sender.is_null());

    log::info!("extern api: mirror sender get sources: kind={:?}", kind);

    let mut items = ManuallyDrop::new(
        unsafe { &*sender }
            .0
            .get_sources(kind.into())
            .unwrap_or_else(|_| Vec::new())
            .into_iter()
            .map(|item| Source {
                index: item.index,
                kind: SourceType::from(item.kind),
                id: CString::new(item.id).unwrap().into_raw(),
                name: CString::new(item.name).unwrap().into_raw(),
            })
            .collect::<Vec<Source>>(),
    );

    Sources {
        items: items.as_mut_ptr(),
        capacity: items.capacity(),
        size: items.len(),
    }
}

/// Because `Sources` are allocated internally, they also need to be released
/// internally.
#[no_mangle]
#[cfg(not(target_os = "macos"))]
pub extern "C" fn mirror_sources_destroy(sources: *const Sources) {
    assert!(!sources.is_null());

    let sources = unsafe { &*sources };
    for item in unsafe { Vec::from_raw_parts(sources.items, sources.size, sources.capacity) } {
        drop(unsafe { CString::from_raw(item.id as *mut _) });
        drop(unsafe { CString::from_raw(item.name as *mut _) });
    }
}

/// Set video capture sources to sender.
#[no_mangle]
#[cfg(not(target_os = "macos"))]
pub extern "C" fn mirror_sender_set_video_source(
    sender: *const Sender,
    source: *const Source,
) -> bool {
    assert!(!sender.is_null() && !source.is_null());

    log::info!("extern api: mirror sender set video source");

    let func = || {
        let source = unsafe { &*source };
        unsafe { &*sender }.0.set_video_source(capture::Source {
            id: Strings::from(source.id).to_string()?,
            name: Strings::from(source.name).to_string()?,
            kind: source.kind.into(),
            index: source.index,
        })
    };

    checker(func()).is_ok()
}

/// Set whether the sender uses multicast transmission.
#[no_mangle]
#[cfg(not(target_os = "macos"))]
pub extern "C" fn mirror_sender_set_multicast(sender: *const Sender, is_multicast: bool) {
    assert!(!sender.is_null());

    log::info!("extern api: mirror set sender multicast={}", is_multicast);
    unsafe { &*sender }.0.set_multicast(is_multicast);
}

/// Get whether the sender uses multicast transmission.
#[no_mangle]
#[cfg(not(target_os = "macos"))]
pub extern "C" fn mirror_sender_get_multicast(sender: *const Sender) -> bool {
    assert!(!sender.is_null());

    log::info!("extern api: mirror get sender multicast");
    unsafe { &*sender }.0.get_multicast()
}

/// Close sender.
#[no_mangle]
#[cfg(not(target_os = "macos"))]
pub extern "C" fn mirror_sender_destroy(sender: *const Sender) {
    assert!(!sender.is_null());

    log::info!("extern api: mirror close sender");
    drop(unsafe { Box::from_raw(sender as *mut Sender) })
}

#[repr(C)]
pub struct Receiver(receiver::Receiver);

/// Create a receiver, specify a bound NIC address, you can pass callback to
/// get the sender's screen or sound callback, callback can not be null.
#[no_mangle]
pub extern "C" fn mirror_create_receiver(
    mirror: *const Mirror,
    id: c_int,
    codec: *const c_char,
    sink: FrameSink,
) -> *const Receiver {
    assert!(!mirror.is_null() && !codec.is_null());

    log::info!("extern api: mirror create receiver");

    let func = || {
        unsafe { &*mirror }.0.create_receiver(
            id as u32,
            receiver::ReceiverOptions {
                video: Strings::from(codec).to_string()?,
                audio: "libopus".to_string(),
            },
            sink.into(),
        )
    };

    checker(func())
        .map(|receiver| Box::into_raw(Box::new(Receiver(receiver))))
        .unwrap_or_else(|_| null_mut())
}

/// Close receiver.
#[no_mangle]
pub extern "C" fn mirror_receiver_destroy(receiver: *const Receiver) {
    assert!(!receiver.is_null());

    log::info!("extern api: mirror close receiver");
    drop(unsafe { Box::from_raw(receiver as *mut Receiver) })
}

#[inline]
fn checker<T, E: Debug>(result: Result<T, E>) -> Result<T, E> {
    if let Err(e) = &result {
        log::error!("{:?}", e);
    }

    result
}
