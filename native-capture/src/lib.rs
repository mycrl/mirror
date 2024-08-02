use anyhow::Result;

pub mod camera;
pub mod screen;

pub trait CaptureFrameHandler: Sync + Send {
    /// The type of data captured, such as video frames.
    type Frame;

    /// This method is called when the capture source captures new data. If it
    /// returns false, the source stops capturing.
    fn sink(&self, frame: &Self::Frame) -> bool;
}

pub trait CaptureHandler: Sync + Send {
    type Error;

    /// The type of data captured, such as video frames.
    type Frame;

    /// Start capturing configuration information, which may be different for
    /// each source.
    type CaptureOptions;

    /// Stop capturing the current source.
    fn stop(&self) -> Result<(), Self::Error>;

    /// Get a list of sources, such as multiple screens in a display source.
    fn get_sources(&self) -> Result<Vec<Source>, Self::Error>;

    /// Start capturing. This function will not block until capturing is
    /// stopped, and it maintains its own capture thread internally.
    fn start<S: CaptureFrameHandler<Frame = Self::Frame> + 'static>(
        &self,
        options: Self::CaptureOptions,
        sink: S,
    ) -> Result<(), Self::Error>;
}

#[derive(Debug, Clone)]
pub struct Source {
    pub id: String,
    pub name: String,
    pub index: usize,
}

#[derive(Debug, Clone)]
pub struct VideoCaptureSourceDescription {
    pub source: Source,
    pub width: u32,
    pub height: u32,
    pub fps: u8,
}
