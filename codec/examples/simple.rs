#![allow(unused)]

use std::{
    collections::HashMap,
    fs::File,
    io::Read,
    ptr::null,
    sync::{mpsc::channel, Arc, Mutex},
    thread,
    time::{Duration, Instant},
};

use clap::Parser;
use common::frame::{VideoFrame, VideoFrameRect};

#[cfg(any(target_os = "windows", target_os = "linux"))]
use codec::{
    video::{find_video_decoder, find_video_encoder},
    VideoDecoder, VideoEncoder, VideoEncoderSettings,
};

#[derive(Parser, Clone)]
#[command(
    about = env!("CARGO_PKG_DESCRIPTION"),
    version = env!("CARGO_PKG_VERSION"),
    author = env!("CARGO_PKG_AUTHORS"),
)]
struct Args {
    #[arg(long)]
    input: String,
    #[arg(long)]
    width: u32,
    #[arg(long)]
    height: u32,
    #[arg(long)]
    fps: u8,
}

#[cfg(any(target_os = "windows", target_os = "linux"))]
fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    let (tx, rx) = channel::<Vec<u8>>();
    let map: Arc<Mutex<HashMap<usize, Instant>>> = Arc::new(Mutex::new(HashMap::new()));

    let map_ = map.clone();
    thread::spawn(move || {
        let mut index = 0;
        let decoder = VideoDecoder::new(&find_video_decoder()).unwrap();

        while let Ok(packet) = rx.recv() {
            if decoder.decode(&packet) {
                while let Some(frame) = decoder.read() {
                    if let Some(time) = map_.lock().unwrap().remove(&index) {
                        println!(
                            "decode frame: {}, delay: {}",
                            index,
                            time.elapsed().as_millis()
                        );
                    }

                    index += 1;
                }
            }
        }
    });

    let encoder = VideoEncoder::new(&VideoEncoderSettings {
        codec_name: find_video_encoder(),
        bit_rate: 500 * 1024 * 8,
        width: args.width,
        height: args.height,
        frame_rate: args.fps,
        key_frame_interval: args.fps as u32,
    })?;

    let mut frame = VideoFrame {
        linesize: [args.width as usize, args.width as usize],
        data: [null(), null()],
        rect: VideoFrameRect {
            width: args.width as usize,
            height: args.height as usize,
        },
    };

    let mut index = 0;
    let mut buf = vec![0u8; (args.width as f32 * args.height as f32 * 1.5) as usize];
    let mut input = File::open(args.input)?;

    while let Ok(_) = input.read_exact(&mut buf) {
        frame.data[0] = buf.as_ptr();
        frame.data[1] = unsafe { buf.as_ptr().add(args.width as usize * args.height as usize) };

        map.lock().unwrap().insert(index, Instant::now());
        index += 1;

        if encoder.encode(&frame) {
            while let Some(packet) = encoder.read() {
                tx.send(packet.buffer.to_vec()).unwrap();
            }
        } else {
            break;
        }

        thread::sleep(Duration::from_millis(1000 / args.fps as u64));
    }

    Ok(())
}

#[cfg(target_os = "macos")]
fn main() {}
