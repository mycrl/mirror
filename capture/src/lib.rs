mod device;
mod manager;

use std::{
    ffi::{c_int, c_void},
    ptr::null,
    sync::Arc,
};

use common::frame::VideoFrame;

pub use device::{Device, DeviceKind, DeviceList};
pub use manager::DeviceManager;

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

extern "C" {
    /// Releases all data associated with OBS and terminates the OBS
    /// context.
    pub fn capture_quit();
    /// Initializes the OBS core context.
    pub fn capture_init(video_info: *const VideoInfo, audio_info: *const AudioInfo) -> c_int;
    /// Adds/removes a raw video callback. Allows the ability to obtain raw
    /// video frames without necessarily using an output.
    pub fn capture_set_video_output_callback(
        proc: Option<extern "C" fn(ctx: *const c_void, frame: *const VideoFrame)>,
        ctx: *const c_void,
    ) -> *const c_void;
}

#[derive(Debug)]
pub enum DeviceError {
    InitializeFailed,
    CreateDeviceManagerFailed,
}

impl std::error::Error for DeviceError {}

impl std::fmt::Display for DeviceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                Self::CreateDeviceManagerFailed => "CreateDeviceManagerFailed",
                Self::InitializeFailed => "InitializeFailed",
            }
        )
    }
}

pub trait VideoSink {
    /// This function is called when obs pushes frames internally, and the
    /// format of the video frame is fixed to NV12.
    ///
    /// ```
    /// struct FrameSink {
    ///     frame: Arc<Mutex<Vec<u8>>>,
    /// }
    ///
    /// impl VideoSink for FrameSink {
    ///     fn sink(&self, frmae: &VideoFrame) {
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
    fn sink(&self, frmae: &VideoFrame);
}

struct Context(Arc<dyn VideoSink>);

extern "C" fn video_sink_proc(ctx: *const c_void, frame: *const VideoFrame) {
    if !ctx.is_null() {
        unsafe { &*(ctx as *const Context) }
            .0
            .sink(unsafe { &*frame });
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
/// impl VideoSink for FrameSink {
///     fn sink(&self, frmae: &VideoFrame) {
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
/// set_video_sink(FrameSink { frame });
/// ```
pub fn set_video_sink<S: VideoSink + 'static>(sink: S) {
    let previous = unsafe {
        capture_set_video_output_callback(
            Some(video_sink_proc),
            Box::into_raw(Box::new(Context(Arc::new(sink)))) as *const c_void,
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
    if unsafe { capture_init(&options.video, &options.audio) } != 0 {
        Err(DeviceError::InitializeFailed)
    } else {
        Ok(())
    }
}

/// Cleans up the OBS environment, a step that needs to be called when the
/// application exits.
pub fn quit() {
    unsafe { capture_quit() }

    let previous = unsafe { capture_set_video_output_callback(None, null()) };
    if !previous.is_null() {
        drop(unsafe { Box::from_raw(previous as *mut Context) })
    }
}
