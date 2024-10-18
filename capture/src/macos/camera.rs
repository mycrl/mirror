use crate::{CaptureHandler, FrameArrived, Source, VideoCaptureSourceDescription};

use mirror_common::frame::VideoFrame;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum CameraCaptureError {}

#[derive(Default)]
pub struct CameraCapture;

impl CaptureHandler for CameraCapture {
    type Frame = VideoFrame;
    type Error = CameraCaptureError;
    type CaptureDescriptor = VideoCaptureSourceDescription;

    fn get_sources() -> Result<Vec<Source>, Self::Error> {
        todo!("camera capture is not supported on macos")
    }

    fn start<S: FrameArrived<Frame = Self::Frame> + 'static>(
        &self,
        _options: Self::CaptureDescriptor,
        mut _arrived: S,
    ) -> Result<(), Self::Error> {
        todo!("camera capture is not supported on macos")
    }

    fn stop(&self) -> Result<(), Self::Error> {
        todo!("camera capture is not supported on macos")
    }
}
