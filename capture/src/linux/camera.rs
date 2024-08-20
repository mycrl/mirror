use crate::{CaptureHandler, FrameArrived, Source, SourceType, VideoCaptureSourceDescription};

use anyhow::anyhow;
use eye_hal::{
    format::PixelFormat,
    traits::{Context, Device},
    PlatformContext,
};
use frame::VideoFrame;

#[derive(Default)]
pub struct CameraCapture();

impl CaptureHandler for CameraCapture {
    type Frame = VideoFrame;
    type Error = anyhow::Error;
    type CaptureOptions = VideoCaptureSourceDescription;

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
        options: Self::CaptureOptions,
        arrived: S,
    ) -> Result<(), Self::Error> {
        let ctx = PlatformContext::default();
        let mut devices = ctx
            .open_device(&options.source.id)?
            .streams()?
            .into_iter()
            .filter(|it| it.pixfmt == PixelFormat::Custom("YUYV".to_string()))
            .filter(|it| it.width <= options.size.width && it.height <= options.size.height)
            .collect::<Vec<_>>();
        devices.sort_by(|a, b| a.width.partial_cmp(&b.width).unwrap());
        let device = devices
            .first()
            .ok_or_else(|| anyhow!("not found a device"))?;

        Ok(())
    }

    fn stop(&self) -> Result<(), Self::Error> {
        todo!()
    }
}
