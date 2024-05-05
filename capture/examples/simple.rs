use std::{
    sync::{Arc, Mutex},
    thread,
    time::Duration,
};

use capture::*;
use common::frame::VideoFrame;
use minifb::{Window, WindowOptions};

const WIDTH: usize = 1280;
const HEIGHT: usize = 720;

struct FrameSink {
    frame: Arc<Mutex<Vec<u8>>>,
}

impl AVFrameSink for FrameSink {
    fn video(&self, frmae: &VideoFrame) {
        let mut frame_ = self.frame.lock().unwrap();

        unsafe {
            libyuv::nv12_to_argb(
                frmae.data[0],
                frmae.linesize[0] as i32,
                frmae.data[1],
                frmae.linesize[1] as i32,
                frame_.as_mut_ptr(),
                WIDTH as i32 * 4,
                WIDTH as i32,
                HEIGHT as i32,
            );
        }
    }
}

fn main() -> anyhow::Result<()> {
    {
        let mut path = std::env::current_exe()?;
        path.pop();
        std::env::set_current_dir(path)?;
    }

    let frame = Arc::new(Mutex::new(vec![0u8; WIDTH * HEIGHT * 4]));
    init(DeviceManagerOptions {
        video: VideoInfo {
            fps: 30,
            width: WIDTH as u32,
            height: HEIGHT as u32,
        },
        audio: AudioInfo {
            samples_per_sec: 48000,
        },
    })?;

    let devices = DeviceManager::get_devices(DeviceKind::Screen).to_vec();
    for device in &devices {
        println!("> Device: name={:?}, id={:?}", device.name(), device.id());
    }

    DeviceManager::set_input(&devices[0]);
    set_frame_sink(FrameSink {
        frame: frame.clone(),
    });

    let mut window = Window::new("simple", WIDTH, HEIGHT, WindowOptions::default())?;
    window.limit_update_rate(Some(Duration::from_millis(1000 / 30)));

    loop {
        {
            let g_frame = frame.lock().unwrap();
            let (_, shorts, _) = unsafe { g_frame.align_to::<u32>() };
            window.update_with_buffer(shorts, WIDTH, HEIGHT)?;
            drop(g_frame);
        }

        thread::sleep(Duration::from_millis(1000 / 30));
    }
}
