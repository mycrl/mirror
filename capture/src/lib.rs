mod device;
mod manager;

use std::{
    ffi::{c_int, c_void},
    ptr::null,
    sync::Arc,
};

use common::frame::{AudioFrame, VideoFrame};

pub use device::{Device, DeviceKind, DeviceList};
pub use manager::DeviceManager;
use num_enum::TryFromPrimitive;

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct VideoInfo {
    pub fps: u8,
    pub width: u32,
    pub height: u32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct AudioInfo {
    pub samples_per_sec: u32,
}

#[repr(C)]
struct RawOutputCallback {
    video: Option<extern "C" fn(ctx: *const c_void, frame: *const VideoFrame)>,
    audio: Option<extern "C" fn(ctx: *const c_void, frame: *const AudioFrame)>,
    ctx: *const c_void,
}

extern "C" {
    /// Initializes the OBS core context.
    fn capture_init(video_info: *const VideoInfo, audio_info: *const AudioInfo) -> c_int;
    /// Adds/removes a raw video callback. Allows the ability to obtain raw
    /// video frames without necessarily using an output.
    fn capture_set_output_callback(proc: RawOutputCallback) -> *const c_void;
    /// Start capturing audio and video data.
    fn capture_start();
    /// Stop capturing audio and video data.
    fn capture_stop();
}

#[derive(Debug, TryFromPrimitive)]
#[repr(i32)]
pub enum DeviceError {
    InitializeFailed = -1,
    StartupFailed = -2,
    ResetVideoFailed = -3,
    ResetAudioFailed = -4,
    CreateSceneFailed = -5,
    CreateWindowDeviceFailed = -6,
    CreateWindowItemFailed = -7,
    CreateMonitorDeviceFailed = -8,
    CreateMonitorItemFailed = -9,
    CreateVideoDeviceFailed = -10,
    CreateVideoItemFailed = -11,
    CreateDefaultAudioDeviceFailed = -12,
    CreateAudioDeviceFailed = -13,
}

impl std::error::Error for DeviceError {}

impl std::fmt::Display for DeviceError {
    #[rustfmt::skip]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                Self::InitializeFailed => "InitializeFailed",
                Self::StartupFailed => "StartupFailed",
                Self::ResetVideoFailed => "ResetVideoFailed",
                Self::ResetAudioFailed => "ResetAudioFailed",
                Self::CreateSceneFailed => "CreateSceneFailed",
                Self::CreateWindowDeviceFailed => "CreateWindowDeviceFailed",
                Self::CreateWindowItemFailed => "CreateWindowItemFailed",
                Self::CreateMonitorDeviceFailed => "CreateMonitorDeviceFailed",
                Self::CreateMonitorItemFailed => "CreateMonitorItemFailed",
                Self::CreateVideoDeviceFailed => "CreateVideoDeviceFailed",
                Self::CreateVideoItemFailed => "CreateVideoItemFailed",
                Self::CreateDefaultAudioDeviceFailed => "CreateDefaultAudioDeviceFailed",
                Self::CreateAudioDeviceFailed => "CreateAudioDeviceFailed",
            }
        )
    }
}

pub trait AVFrameSink {
    /// This function is called when obs pushes frames internally, and the
    /// format of the video frame is fixed to NV12.
    ///
    /// ```
    /// struct FrameSink {
    ///     frame: Arc<Mutex<Vec<u8>>>,
    /// }
    ///
    /// impl AVFrameSink for FrameSink {
    ///     fn video(&self, frmae: &VideoFrame) {
    ///         let mut frame_ = self.frame.lock().unwrap();
    ///
    ///         unsafe {
    ///             libyuv::nv12_to_argb(
    ///                 frmae.data[0],
    ///                 frmae.linesize[0] as i32,
    ///                 frmae.data[1],
    ///                 frmae.linesize[1] as i32,
    ///                 frame_.as_mut_ptr(),
    ///                 1920 * 4,
    ///                 1920,
    ///                 1080,
    ///             );
    ///         }
    ///     }
    /// }
    /// ```
    #[allow(unused_variables)]
    fn video(&self, frmae: &VideoFrame) {}
    /// This function is called when obs pushes frames internally, and the
    /// format of the audio frame is fixed to PCM.
    ///
    /// ```
    /// struct FrameSink {
    ///     frame: Arc<Mutex<Vec<u8>>>,
    /// }
    ///
    /// impl AVFrameSink for FrameSink {
    ///     fn audio(&self, frmae: &AudioFrame) {
    ///         let mut frame_ = self.frame.lock().unwrap();
    ///         frame_.clear();
    ///         frame.extend_from_slice(frame.data[0]);
    ///         frame.extend_from_slice(frame.data[1]);
    ///     }
    /// }
    /// ```
    #[allow(unused_variables)]
    fn audio(&self, frame: &AudioFrame) {}
}

impl AVFrameSink for () {}

struct Context(Arc<dyn AVFrameSink>);

extern "C" fn video_sink_proc(ctx: *const c_void, frame: *const VideoFrame) {
    if !ctx.is_null() {
        unsafe { &*(ctx as *const Context) }
            .0
            .video(unsafe { &*frame });
    }
}

extern "C" fn audio_sink_proc(ctx: *const c_void, frame: *const AudioFrame) {
    if !ctx.is_null() {
        unsafe { &*(ctx as *const Context) }
            .0
            .audio(unsafe { &*frame });
    }
}

/// This function is called when obs pushes frames internally, and the
/// format of the video frame is fixed to NV12.
///
/// ```
/// struct FrameSink {
///     frame: Arc<Mutex<Vec<u8>>>,
/// }
///
/// impl AVFrameSink for FrameSink {
///     fn video(&self, frmae: &VideoFrame) {
///         let mut frame_ = self.frame.lock().unwrap();
///
///         unsafe {
///             libyuv::nv12_to_argb(
///                 frmae.data[0],
///                 frmae.linesize[0] as i32,
///                 frmae.data[1],
///                 frmae.linesize[1] as i32,
///                 frame_.as_mut_ptr(),
///                 1920 * 4,
///                 1920,
///                 1080,
///             );
///         }
///     }
/// }
///
/// let frame = Arc::new(Mutex::new(vec![0u8; (1920 * 1080 * 4) as usize]));
/// set_frame_sink(FrameSink { frame });
/// ```
pub fn set_frame_sink<S: AVFrameSink + 'static>(sink: Option<S>) {
    let previous = unsafe {
        capture_set_output_callback(
            sink.map(|it| RawOutputCallback {
                ctx: Box::into_raw(Box::new(Context(Arc::new(it)))) as *const c_void,
                video: Some(video_sink_proc),
                audio: Some(audio_sink_proc),
            })
            .unwrap_or_else(|| RawOutputCallback {
                video: None,
                audio: None,
                ctx: null(),
            }),
        )
    };

    if !previous.is_null() {
        drop(unsafe { Box::from_raw(previous as *mut Context) })
    }
}

#[derive(Debug, Clone)]
pub struct DeviceManagerOptions {
    pub video: VideoInfo,
    pub audio: AudioInfo,
}

/// Initialize the OBS environment, this is a required step, before calling any
/// function.
///
/// ```
/// init(DeviceManagerOptions {
///     video: VideoInfo {
///         fps: 30,
///         width: WIDTH as u32,
///         height: HEIGHT as u32,
///     },
/// })?;
/// ```
pub fn init(options: DeviceManagerOptions) -> Result<(), DeviceError> {
    let result = unsafe { capture_init(&options.video, &options.audio) };
    if result != 0 {
        Err(DeviceError::try_from(result).unwrap())
    } else {
        Ok(())
    }
}

/// Start capturing audio and video data.
pub fn start() {
    unsafe { capture_start() }
}

/// Stop capturing audio and video data.
pub fn stop() {
    unsafe { capture_stop() }
    set_frame_sink::<()>(None);
}