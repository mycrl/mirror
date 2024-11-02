use crate::{CaptureHandler, FrameArrived, Source, VideoCaptureSourceDescription};

use hylarana_common::frame::VideoFrame;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ScreenCaptureError {}

#[derive(Default)]
pub struct ScreenCapture;

impl CaptureHandler for ScreenCapture {
    type Frame = VideoFrame;
    type Error = ScreenCaptureError;
    type CaptureDescriptor = VideoCaptureSourceDescription;

    fn get_sources() -> Result<Vec<Source>, Self::Error> {
        Ok(Vec::new())
    }

    fn start<S: FrameArrived<Frame = Self::Frame> + 'static>(
        &self,
        _options: Self::CaptureDescriptor,
        mut _arrived: S,
    ) -> Result<(), Self::Error> {
        todo!("screen capture is not supported on macos")
    }

    fn stop(&self) -> Result<(), Self::Error> {
        todo!("screen capture is not supported on macos")
    }
}
