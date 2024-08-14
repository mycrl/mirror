use std::{
    sync::{atomic::AtomicBool, Arc},
    thread,
};

use anyhow::Result;
use frame::VideoFrame;
use scap::{
    capturer::{Capturer, Options, Resolution},
    frame::FrameType,
    get_all_targets, Target,
};
use utils::atomic::EasyAtomic;

use crate::{CaptureHandler, FrameArrived, Source, SourceType, VideoCaptureSourceDescription};

#[derive(Default)]
pub struct ScreenCapture(Arc<AtomicBool>);

impl CaptureHandler for ScreenCapture {
    type Frame = VideoFrame;
    type Error = anyhow::Error;
    type CaptureOptions = VideoCaptureSourceDescription;

    fn get_sources() -> Result<Vec<Source>, Self::Error> {
        let mut sources = Vec::with_capacity(5);
        for item in get_all_targets() {
            if let Target::Display(item) = item {
                sources.push(Source {
                    name: item.title,
                    id: item.id.to_string(),
                    index: item.id as usize,
                    kind: SourceType::Screen,
                    is_default: true,
                });
            }
        }

        Ok(sources)
    }

    fn start<S: FrameArrived<Frame = Self::Frame> + 'static>(
        &self,
        options: Self::CaptureOptions,
        mut arrived: S,
    ) -> Result<(), Self::Error> {
        let mut capturer = Capturer::new(Options {
            fps: options.fps as u32,
            target: None,
            show_cursor: false,
            show_highlight: true,
            excluded_targets: None,
            output_type: FrameType::YUVFrame,
            output_resolution: Resolution::_720p,
            crop_area: None,
        });

        let status = self.0.clone();
        thread::Builder::new()
            .name("LinuxScreenCaptureThread".to_string())
            .spawn(move || {
                capturer.start_capture();
                status.update(true);

                while let Ok(frame) = capturer.get_next_frame() {
                    if !status.get() {
                        break;
                    }

                    println!("==================");
                }

                capturer.stop_capture();
                status.update(false);
            })?;

        Ok(())
    }

    fn stop(&self) -> Result<(), Self::Error> {
        self.0.update(false);
        Ok(())
    }
}
