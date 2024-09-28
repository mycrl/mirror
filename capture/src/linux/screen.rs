use crate::{
    CaptureHandler, FrameArrived, Size, Source, SourceType, VideoCaptureSourceDescription,
};

use std::{
    env,
    ptr::null_mut,
    sync::{atomic::AtomicBool, Arc},
    thread,
    time::Duration,
};

use anyhow::{anyhow, Result};
use frame::VideoFrame;
use utils::{atomic::EasyAtomic, strings::Strings};

#[derive(Default)]
pub struct ScreenCapture(Arc<AtomicBool>);

impl CaptureHandler for ScreenCapture {
    type Frame = VideoFrame;
    type Error = anyhow::Error;
    type CaptureDescriptor = VideoCaptureSourceDescription;

    fn get_sources() -> Result<Vec<Source>, Self::Error> {
        Ok(vec![Source {
            index: 0,
            is_default: true,
            kind: SourceType::Screen,
            id: "default display".to_string(),
            name: "default display".to_string(),
        }])
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

struct Capture(*mut AVFormatContext);

unsafe impl Send for Capture {}
unsafe impl Sync for Capture {}

impl Capture {
    fn new() -> Result<Self> {
        let mut ctx = null_mut();
        if unsafe {
            avformat_open_input(
                &mut ctx,
                "/dev/dri/card1".as_ptr() as *const _,
                av_find_input_format("kmsgrab".as_ptr() as *const _),
                null_mut(),
            )
        } != 0
        {
            return Err(anyhow!("not open kms device"));
        }

        if unsafe { avformat_find_stream_info(ctx, null_mut()) } != 0 {
            return Err(anyhow!("not found kms device capture stream"));
        }

        Ok(Self(ctx))
    }
}

impl Drop for Capture {
    fn drop(&mut self) {
        unsafe {
            avformat_close_input(&mut self.0);
        }
    }
}
