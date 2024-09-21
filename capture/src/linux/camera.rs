use crate::{CaptureHandler, FrameArrived, Source, SourceType, VideoCaptureSourceDescription};

use frame::VideoFrame;

#[derive(Default)]
pub struct CameraCapture();

impl CaptureHandler for CameraCapture {
    type Frame = VideoFrame;
    type Error = anyhow::Error;
    type CaptureDescriptor = VideoCaptureSourceDescription;

    fn get_sources() -> Result<Vec<Source>, Self::Error> {
       todo!()
    }

    fn start<S: FrameArrived<Frame = Self::Frame> + 'static>(
        &self,
        options: Self::CaptureDescriptor,
        arrived: S,
    ) -> Result<(), Self::Error> {
        todo!()
    }

    fn stop(&self) -> Result<(), Self::Error> {
        todo!()
    }
}
