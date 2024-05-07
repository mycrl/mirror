#![allow(unused)]

use std::{thread, time::Duration};

use codec::{video::find_video_encoder, VideoEncoder, VideoEncoderSettings};
use common::frame::{VideoFrame, VideoFrameRect};

#[cfg(any(target_os = "windows", target_os = "linux"))]
fn main() -> anyhow::Result<()> {
    let encoder = VideoEncoder::new(&VideoEncoderSettings {
        codec_name: find_video_encoder(),
        max_b_frames: 0,
        width: 1280,
        height: 720,
        frame_rate: 30,
        bit_rate: 500 * 1024 * 8,
        key_frame_interval: 15,
    })?;

    let buf = vec![0u8; 1280 * 720 * 4];
    let frame = VideoFrame {
        linesize: [1280, 1280],
        rect: VideoFrameRect {
            width: 1280,
            height: 720,
        },
        data: [
            buf.as_slice().as_ptr(),
            buf.as_slice().as_ptr(),
        ]
    };

    loop {
        if encoder.encode(&frame) {
            while let Some(_packet) = encoder.read() {

            }
        } else {
            break;
        }

        thread::sleep(Duration::from_millis(1000 / 30));
    }

    Ok(())
}

#[cfg(target_os = "macos")]
fn main() {}
