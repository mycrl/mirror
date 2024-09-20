use crate::{CaptureHandler, FrameArrived, Source, SourceType, VideoCaptureSourceDescription};

use anyhow::anyhow;
use frame::VideoFrame;

#[derive(Default)]
pub struct CameraCapture();

impl CaptureHandler for CameraCapture {
    type Frame = VideoFrame;
    type Error = anyhow::Error;
    type CaptureDescriptor = VideoCaptureSourceDescription;

    fn get_sources() -> Result<Vec<Source>, Self::Error> {
        Ok(PlatformContext::default()
            .devices()?
            .into_iter()
            .enumerate()
            .map(|(index, it)| Source {
                index,
                id: it.uri,
                name: it.product,
                kind: SourceType::Camera,
                is_default: index == 0,
            })
            .collect())
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
