use std::time::Duration;

use common::frame::VideoFrame;
use native_capture::{
    screen::ScreenCapture, CaptureFrameHandler, CaptureHandler, VideoCaptureSourceDescription,
};

struct Sink {}

impl CaptureFrameHandler for Sink {
    type Frame = VideoFrame;

    fn sink(&self, frame: &Self::Frame) -> bool {
        println!("===================== {:#?}", frame);
        true
    }
}

fn main() {
    let capture = ScreenCapture::new().unwrap();
    let sources = capture.get_sources().unwrap();
    println!("{:#?}", sources);

    capture.start(
        VideoCaptureSourceDescription {
            source: sources[0].clone(),
            width: 1280,
            height: 720,
            fps: 2,
        },
        Sink {},
    ).unwrap();

    std::thread::sleep(Duration::from_secs(60));
}
