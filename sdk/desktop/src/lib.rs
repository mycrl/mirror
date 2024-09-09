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
use std::{
    ffi::{c_void, CString},
    mem::ManuallyDrop,
};

use codec::{VideoDecoderType, VideoEncoderType};
use frame::{AudioFrame, VideoFrame};
use utils::{atomic::EasyAtomic, strings::Strings};

#[cfg(not(target_os = "macos"))]
use capture::{Capture, SourceType};

#[cfg(target_os = "windows")]
use utils::win32::Interface;

/// Windows yes! The Windows dynamic library has an entry, so just initialize
/// the logger and set the process priority at the entry.
#[no_mangle]
#[cfg(target_os = "windows")]
extern "system" fn DllMain(
    _module: u32,
    call_reason: usize,
    reserved: *const std::ffi::c_void,
) -> bool {
    match call_reason {
        1 /* DLL_PROCESS_ATTACH */ => mirror_startup(),
        0 /* DLL_PROCESS_DETACH */ => {
            if reserved.is_null() {
                mirror_shutdown();
            }

            true
        },
        _ => true,
    }
}

/// Initialize the environment, which must be initialized before using the SDK.
#[no_mangle]
pub extern "C" fn mirror_startup() -> bool {
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
pub struct MirrorDescriptor {
    pub server: *const c_char,
    pub multicast: *const c_char,
    pub mtu: usize,
}

impl TryInto<transport::TransportDescriptor> for MirrorDescriptor {
    type Error = anyhow::Error;

    fn try_into(self) -> Result<transport::TransportDescriptor, Self::Error> {
        Ok(transport::TransportDescriptor {
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
pub extern "C" fn mirror_create(options: MirrorDescriptor) -> *const Mirror {
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

/// Get direct3d device.
#[no_mangle]
#[cfg(target_os = "windows")]
pub extern "C" fn mirror_get_direct3d_device(mirror: *const Mirror) -> *mut c_void {
    assert!(!mirror.is_null());

    unsafe { &*mirror }
        .0
        .get_direct3d_device()
        .map(|it| it.device.as_raw())
        .unwrap_or_else(|| null_mut())
}

/// Get direct3d device context.
#[no_mangle]
#[cfg(target_os = "windows")]
pub extern "C" fn mirror_get_direct3d_device_context(mirror: *const Mirror) -> *mut c_void {
    assert!(!mirror.is_null());

    unsafe { &*mirror }
        .0
        .get_direct3d_device()
        .map(|it| it.context.as_raw())
        .unwrap_or_else(|| null_mut())
}

#[repr(C)]
#[derive(Debug)]
#[cfg(not(target_os = "macos"))]
pub struct Source {
    index: usize,
    kind: SourceType,
    id: *const c_char,
    name: *const c_char,
    is_default: bool,
}

#[cfg(not(target_os = "macos"))]
impl TryInto<capture::Source> for &Source {
    type Error = anyhow::Error;

    fn try_into(self) -> Result<capture::Source, Self::Error> {
        Ok(capture::Source {
            name: Strings::from(self.name).to_string()?,
            id: Strings::from(self.id).to_string()?,
            is_default: self.is_default,
            index: self.index,
            kind: self.kind,
        })
    }
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
pub extern "C" fn mirror_get_sources(kind: SourceType) -> Sources {
    log::info!("extern api: mirror get sources: kind={:?}", kind);

    let mut items = ManuallyDrop::new(
        Capture::get_sources(kind.into())
            .unwrap_or_else(|_| Vec::new())
            .into_iter()
            .map(|item| {
                log::info!("source: {:?}", item);

                Source {
                    index: item.index,
                    is_default: item.is_default,
                    kind: SourceType::from(item.kind),
                    id: CString::new(item.id).unwrap().into_raw(),
                    name: CString::new(item.name).unwrap().into_raw(),
                }
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
        // Record whether it is closed
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
                // I thought about it carefully. The closing hand should only trigger the
                // callback once. There are too many places in the system that will trigger the
                // closing callback. It is not easy to manage the status between components.
                // Here, the closing status is directly recorded. If it has been closed, it will
                // not be processed anymore.
                if !is_closed.get() {
                    is_closed.update(true);

                    if let Some(callback) = &self.close {
                        callback(self.ctx);

                        log::info!("extern api: call close callback");
                    }
                }
            }),
        }
    }
}

/// Video Codec Configuretion.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct VideoDescriptor {
    pub codec: *const c_char,
    pub frame_rate: u8,
    pub width: u32,
    pub height: u32,
    pub bit_rate: u64,
    pub key_frame_interval: u32,
}

#[cfg(not(target_os = "macos"))]
impl TryInto<crate::sender::VideoDescriptor> for VideoDescriptor {
    type Error = anyhow::Error;

    fn try_into(self) -> Result<crate::sender::VideoDescriptor, Self::Error> {
        Ok(crate::sender::VideoDescriptor {
            codec: VideoEncoderType::try_from(Strings::from(self.codec).to_string()?.as_str())?,
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
pub struct AudioDescriptor {
    pub sample_rate: u64,
    pub bit_rate: u64,
}

#[cfg(not(target_os = "macos"))]
impl Into<crate::sender::AudioDescriptor> for AudioDescriptor {
    fn into(self) -> crate::sender::AudioDescriptor {
        crate::sender::AudioDescriptor {
            sample_rate: self.sample_rate,
            bit_rate: self.bit_rate,
        }
    }
}

#[repr(C)]
#[derive(Debug)]
#[cfg(not(target_os = "macos"))]
pub struct SenderSourceDescriptor<T> {
    source: *const Source,
    options: T,
}

#[repr(C)]
#[derive(Debug)]
#[cfg(not(target_os = "macos"))]
pub struct SenderDescriptor {
    video: *const SenderSourceDescriptor<VideoDescriptor>,
    audio: *const SenderSourceDescriptor<AudioDescriptor>,
    multicast: bool,
}

#[cfg(not(target_os = "macos"))]
impl TryInto<sender::SenderDescriptor> for SenderDescriptor {
    type Error = anyhow::Error;

    // Both video and audio are optional, so the type conversion here is a bit more
    // complicated.
    #[rustfmt::skip]
    fn try_into(self) -> Result<sender::SenderDescriptor, Self::Error> {
        let mut options = sender::SenderDescriptor {
            multicast: self.multicast,
            audio: None,
            video: None,
        };

        if !self.video.is_null() {
            let video = unsafe { &*self.video };
            let settings: crate::sender::VideoDescriptor = video.options.try_into()?;

            // Check whether the external parameters are configured correctly to 
            // avoid some clowns inserting some inexplicable parameters.
            anyhow::ensure!(settings.width % 4 == 0 && settings.width <= 4096, "invalid video width");
            anyhow::ensure!(settings.height % 4 == 0 && settings.height <= 2560, "invalid video height");
            anyhow::ensure!(settings.frame_rate <= 60, "invalid video frame rate");

            options.video = Some((
                unsafe { &*video.source }.try_into()?,
                settings,
            ));
        }

        if !self.audio.is_null() {
            let audio = unsafe { &*self.audio };
            options.audio = Some((
                unsafe { &*audio.source }.try_into()?,
                audio.options.try_into()?,
            ));
        }

        Ok(options)
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
    options: SenderDescriptor,
    sink: FrameSink,
) -> *const Sender {
    assert!(!mirror.is_null());

    log::info!("extern api: mirror create sender");

    let func = || {
        let options: sender::SenderDescriptor = options.try_into()?;
        log::info!("mirror create options={:?}", options);
        
        unsafe { &*mirror }
            .0
            .create_sender(id as u32, options, sink.into())
    };

    checker(func())
    .map(|sender| Box::into_raw(Box::new(Sender(sender))))
    .unwrap_or_else(|_| null_mut())
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
            receiver::ReceiverDescriptor {
                video: VideoDecoderType::try_from(Strings::from(codec).to_string()?.as_str())?,
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

// In fact, this is a package that is convenient for recording errors. If the
// result is an error message, it is output to the log. This function does not
// make any changes to the result.
#[inline]
fn checker<T, E: Debug>(result: Result<T, E>) -> Result<T, E> {
    if let Err(e) = &result {
        log::error!("{:?}", e);

        if cfg!(debug_assertions) {
            println!("{:#?}", e);
        }
    }

    result
}
