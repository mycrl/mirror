#![allow(unused)]

use std::{ffi::CString, ptr::null, thread, time::Duration};

#[cfg(any(target_os = "windows", target_os = "linux"))]
use capture::DeviceKind;
use clap::Parser;

#[cfg(any(target_os = "windows", target_os = "linux"))]
use codec::video::{codec_find_video_decoder, codec_find_video_encoder};

#[cfg(any(target_os = "windows", target_os = "linux"))]
use mirror::{
    mirror_close_receiver, mirror_close_sender, mirror_create, mirror_create_receiver,
    mirror_create_sender, mirror_drop, mirror_drop_devices, mirror_get_devices, mirror_init,
    mirror_quit, mirror_set_input_device, RawAudioOptions, RawFrameSink, RawMirrorOptions,
    RawVideoOptions,
};

#[derive(Parser, Clone)]
#[command(
    about = env!("CARGO_PKG_DESCRIPTION"),
    version = env!("CARGO_PKG_VERSION"),
    author = env!("CARGO_PKG_AUTHORS"),
)]
struct Args {
    #[arg(long)]
    server: String,
}

#[cfg(any(target_os = "windows", target_os = "linux"))]
fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    let server = CString::new(args.server)?;
    let multicast = CString::new("239.0.0.1")?;
    mirror_init(RawMirrorOptions {
        multicast: multicast.as_ptr(),
        server: server.as_ptr(),
        mtu: 1400,
        video: RawVideoOptions {
            encoder: unsafe { codec_find_video_encoder() },
            decoder: unsafe { codec_find_video_decoder() },
            frame_rate: 30,
            width: 1920,
            height: 1080,
            bit_rate: 200 * 1024 * 8,
            key_frame_interval: 30,
        },
        audio: RawAudioOptions {
            sample_rate: 48000,
            bit_rate: 64000,
        },
    });

    let mirror = mirror_create();
    let devices = mirror_get_devices(DeviceKind::Screen);
    mirror_set_input_device(&unsafe { std::slice::from_raw_parts(devices.list, devices.size) }[0]);

    mirror_drop_devices(&devices);

    let sender = mirror_create_sender(
        mirror,
        0,
        RawFrameSink {
            video: None,
            audio: None,
            close: None,
            ctx: null(),
        },
    );

    thread::sleep(Duration::from_secs(9999));
    mirror_close_sender(sender);

    mirror_drop(mirror);
    mirror_quit();
    Ok(())
}

#[cfg(target_os = "macos")]
fn main() {}
