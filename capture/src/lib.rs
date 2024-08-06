mod microphone;

#[cfg(target_os = "windows")]
mod win32;

#[cfg(target_os = "windows")]
use self::win32::{camera::CameraCapture, screen::ScreenCapture};

use self::microphone::MicrophoneCapture;

use anyhow::Result;
use common::frame::{AudioFrame, VideoFrame};

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
    type CaptureOptions;

    /// Get a list of sources, such as multiple screens in a display source.
    fn get_sources() -> Result<Vec<Source>, Self::Error>;

    /// Start capturing. This function will not block until capturing is
    /// stopped, and it maintains its own capture thread internally.
    fn start<S: FrameArrived<Frame = Self::Frame> + 'static>(
        &self,
        options: Self::CaptureOptions,
        arrived: S,
    ) -> Result<(), Self::Error>;

    /// Stop capturing the current source.
    fn stop(&self) -> Result<(), Self::Error>;
}

/// Don't forget to initialize the environment, this is necessary for the
/// capture module.
pub fn startup() -> Result<()> {
    #[cfg(target_os = "windows")]
    {
        log::info!("capture MediaFoundation satrtup");

        self::win32::startup()
    }
}

pub fn shutdown() -> Result<()> {
    #[cfg(target_os = "windows")]
    {
        log::info!("capture MediaFoundation shutdown");

        self::win32::shutdown()
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceType {
    Camera = 1,
    Screen = 2,
    Microphone = 3,
}

#[derive(Debug, Clone)]
pub struct Source {
    pub id: String,
    pub name: String,
    pub index: usize,
    pub kind: SourceType,
}

#[derive(Debug, Clone, Copy)]
pub struct Size {
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone)]
pub struct VideoCaptureSourceDescription {
    pub source: Source,
    pub size: Size,
    pub fps: u8,
}

#[derive(Debug, Clone)]
pub struct AudioCaptureSourceDescription {
    pub source: Source,
    pub sample_rate: u32,
}

#[derive(Default)]
pub struct Capture {
    camera: CameraCapture,
    screen: ScreenCapture,
    microphone: MicrophoneCapture,
}

impl Capture {
    /// Returns a list of devices by type.
    pub fn get_sources(kind: SourceType) -> Result<Vec<Source>> {
        log::info!("capture get sources, kind={:?}", kind);

        Ok(match kind {
            SourceType::Camera => CameraCapture::get_sources()?,
            SourceType::Screen => ScreenCapture::get_sources()?,
            SourceType::Microphone => MicrophoneCapture::get_sources()?,
        })
    }

    /// Capture video devices, including screens or webcams.
    pub fn set_video_source<T: FrameArrived<Frame = VideoFrame> + 'static>(
        &self,
        description: VideoCaptureSourceDescription,
        arrived: T,
    ) -> Result<()> {
        log::info!("capture set video source, description={:?}", description);

        match description.source.kind {
            SourceType::Camera => self.camera.start(description, arrived)?,
            SourceType::Screen => self.screen.start(description, arrived)?,
            _ => (),
        }

        Ok(())
    }

    /// Capture audio devices, including microphone or system.
    pub fn set_audio_source<T: FrameArrived<Frame = AudioFrame> + 'static>(
        &self,
        description: AudioCaptureSourceDescription,
        arrived: T,
    ) -> Result<()> {
        log::info!("capture set audio source, description={:?}", description);

        match description.source.kind {
            SourceType::Microphone => self.microphone.start(description, arrived)?,
            _ => (),
        }

        Ok(())
    }

    pub fn close(&self) -> Result<()> {
        log::info!("close capture");

        self.camera.stop()?;
        self.screen.stop()?;
        self.microphone.stop()?;

        Ok(())
    }
}

impl Drop for Capture {
    fn drop(&mut self) {
        log::info!("capture drop");

        drop(self.close());
    }
}
