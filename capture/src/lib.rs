mod audio;

#[cfg(target_os = "windows")]
mod win32 {
    pub mod camera;
    pub mod screen;
}

#[cfg(target_os = "linux")]
mod linux {
    pub mod camera;
    pub mod screen;
}

#[cfg(target_os = "macos")]
mod macos {
    pub mod camera;
    pub mod screen;
}

pub use self::audio::{AudioCapture, AudioCaptureError};

#[cfg(target_os = "windows")]
pub use self::win32::{
    camera::{CameraCapture, CameraCaptureError},
    screen::{ScreenCapture, ScreenCaptureError},
};

#[cfg(target_os = "linux")]
pub use self::linux::{
    camera::{CameraCapture, CameraCaptureError},
    screen::{ScreenCapture, ScreenCaptureError},
};

#[cfg(target_os = "macos")]
pub use self::macos::{
    camera::{CameraCapture, CameraCaptureError},
    screen::{ScreenCapture, ScreenCaptureError},
};

use common::{
    frame::{AudioFrame, VideoFrame},
    Size,
};

use thiserror::Error;

#[cfg(target_os = "windows")]
use common::win32::Direct3DDevice;

#[cfg(target_os = "linux")]
pub fn startup() {
    unsafe {
        ffmpeg_sys_next::avdevice_register_all();
    }
}

#[derive(Debug, Error)]
pub enum CaptureError {
    #[error(transparent)]
    AudioCaptureError(#[from] AudioCaptureError),
    #[error(transparent)]
    ScreenCaptureError(#[from] ScreenCaptureError),
    #[error(transparent)]
    CameraCaptureError(#[from] CameraCaptureError),
}

pub trait FrameArrived: Sync + Send {
    /// The type of data captured, such as video frames.
    type Frame;

    /// This method is called when the capture source captures new data. If it
    /// returns false, the source stops capturing.
    fn sink(&mut self, frame: &Self::Frame) -> bool;
}

pub trait CaptureHandler: Sync + Send {
    type Error;

    /// The type of data captured, such as video frames.
    type Frame;

    /// Start capturing configuration information, which may be different for
    /// each source.
    type CaptureDescriptor;

    /// Get a list of sources, such as multiple screens in a display source.
    fn get_sources() -> Result<Vec<Source>, Self::Error>;

    /// Stop capturing the current source.
    fn stop(&self) -> Result<(), Self::Error>;

    /// Start capturing. This function will not block until capturing is
    /// stopped, and it maintains its own capture thread internally.
    fn start<S: FrameArrived<Frame = Self::Frame> + 'static>(
        &self,
        options: Self::CaptureDescriptor,
        arrived: S,
    ) -> Result<(), Self::Error>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceType {
    Camera,
    Screen,
    Audio,
}

#[derive(Debug, Clone)]
pub struct Source {
    /// Device ID, usually the symbolic link to the device or the address of the
    /// device file handle.
    pub id: String,
    pub name: String,
    /// Sequence number, which can normally be ignored, in most cases this field
    /// has no real meaning and simply indicates the order in which the device
    /// was acquired internally.
    pub index: usize,
    pub kind: SourceType,
    /// Whether or not it is the default device, normally used to indicate
    /// whether or not it is the master device.
    pub is_default: bool,
}

#[derive(Debug, Clone)]
pub struct VideoCaptureSourceDescription {
    #[cfg(target_os = "windows")]
    pub direct3d: Direct3DDevice,
    /// Indicates whether the capturer internally outputs hardware frames or
    /// not, it should be noted that internally it will just output hardware
    /// frames to the best of its ability and may also output software frames.
    pub hardware: bool,
    pub source: Source,
    pub size: Size,
    pub fps: u8,
}

#[derive(Debug, Clone)]
pub struct AudioCaptureSourceDescription {
    pub source: Source,
    pub sample_rate: u32,
}

pub struct SourceCaptureDescriptor<T, P> {
    pub description: P,
    pub arrived: T,
}

pub struct CaptureDescriptor<V, A>
where
    V: FrameArrived<Frame = VideoFrame>,
    A: FrameArrived<Frame = AudioFrame>,
{
    pub video: Option<SourceCaptureDescriptor<V, VideoCaptureSourceDescription>>,
    pub audio: Option<SourceCaptureDescriptor<A, AudioCaptureSourceDescription>>,
}

impl<V, A> Default for CaptureDescriptor<V, A>
where
    V: FrameArrived<Frame = VideoFrame>,
    A: FrameArrived<Frame = AudioFrame>,
{
    fn default() -> Self {
        Self {
            video: None,
            audio: None,
        }
    }
}

enum CaptureImplement {
    Camera(CameraCapture),
    Screen(ScreenCapture),
    Audio(AudioCapture),
}

/// Capture implementations for audio devices and video devices.
#[derive(Default)]
pub struct Capture(Vec<CaptureImplement>);

impl Capture {
    /// Get all sources that can be used for capture by specifying the type,
    /// which is usually an audio or video device.
    #[allow(unreachable_patterns)]
    pub fn get_sources(kind: SourceType) -> Result<Vec<Source>, CaptureError> {
        log::info!("capture get sources, kind={:?}", kind);

        Ok(match kind {
            SourceType::Camera => CameraCapture::get_sources()?,
            SourceType::Screen => ScreenCapture::get_sources()?,
            SourceType::Audio => AudioCapture::get_sources()?,
            _ => Vec::new(),
        })
    }

    pub fn new<V, A>(
        CaptureDescriptor { video, audio }: CaptureDescriptor<V, A>,
    ) -> Result<Self, CaptureError>
    where
        V: FrameArrived<Frame = VideoFrame> + 'static,
        A: FrameArrived<Frame = AudioFrame> + 'static,
    {
        let mut devices = Vec::with_capacity(3);

        if let Some(SourceCaptureDescriptor {
            description,
            arrived,
        }) = video
        {
            match description.source.kind {
                SourceType::Camera => {
                    let camera = CameraCapture::default();
                    camera.start(description, arrived)?;
                    devices.push(CaptureImplement::Camera(camera));
                }
                SourceType::Screen => {
                    let screen = ScreenCapture::default();
                    screen.start(description, arrived)?;
                    devices.push(CaptureImplement::Screen(screen));
                }
                _ => (),
            }
        }

        if let Some(SourceCaptureDescriptor {
            description,
            arrived,
        }) = audio
        {
            let audio = AudioCapture::default();
            audio.start(description, arrived)?;
            devices.push(CaptureImplement::Audio(audio));
        }

        Ok(Self(devices))
    }

    pub fn close(&self) -> Result<(), CaptureError> {
        for item in self.0.iter() {
            match item {
                CaptureImplement::Screen(it) => it.stop()?,
                CaptureImplement::Camera(it) => it.stop()?,
                CaptureImplement::Audio(it) => it.stop()?,
            };
        }

        log::info!("close capture");

        Ok(())
    }
}

impl Drop for Capture {
    fn drop(&mut self) {
        log::info!("capture drop");

        drop(self.close());
    }
}
