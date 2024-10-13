use crate::{CaptureHandler, FrameArrived, Source, SourceType, VideoCaptureSourceDescription};

use std::{
    ptr::{null, null_mut},
    sync::{atomic::AtomicBool, Arc},
    thread::{self, sleep},
    time::Duration,
};

use common::{
    atomic::EasyAtomic,
    c_str,
    frame::{VideoFormat, VideoFrame, VideoSubFormat},
};

use thiserror::Error;

#[derive(Error, Debug)]
pub enum ScreenCaptureError {}

#[derive(Default)]
pub struct ScreenCapture(Arc<AtomicBool>);

impl CaptureHandler for ScreenCapture {
    type Frame = VideoFrame;
    type Error = ScreenCaptureError;
    type CaptureDescriptor = VideoCaptureSourceDescription;

    fn get_sources() -> Result<Vec<Source>, Self::Error> {
        todo!()
    }

    fn start<S: FrameArrived<Frame = Self::Frame> + 'static>(
        &self,
        options: Self::CaptureDescriptor,
        mut arrived: S,
    ) -> Result<(), Self::Error> {
        todo!()
    }

    fn stop(&self) -> Result<(), Self::Error> {
        self.0.update(false);
        Ok(())
    }
}
