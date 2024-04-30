use std::{
    sync::{Arc, RwLock},
    thread,
    time::Duration,
};

use devices::*;
use minifb::{Window, WindowOptions};

const WIDTH: usize = 1280;
const HEIGHT: usize = 720;

fn main() -> anyhow::Result<()> {
    {
        let mut path = std::env::current_exe()?;
        path.pop();
        std::env::set_current_dir(path)?;
    }

    let frame = Arc::new(RwLock::new(vec![0u8; (WIDTH * HEIGHT * 4) as usize]));
    init(DeviceManagerOptions {
        video: VideoInfo {
            fps: 30,
            width: WIDTH as u32,
            height: HEIGHT as u32,
        },
    })?;

    let devices = get_devices(DeviceKind::Screen);
    for device in &devices {
        println!("device: name={:?}, id={:?}", device.name(), device.id());
    }

    set_input(&devices[0]);

    let mut window = Window::new("simple", WIDTH, HEIGHT, WindowOptions::default())?;
    window.limit_update_rate(Some(Duration::from_millis(1000 / 30)));

    loop {
        {
            let g_frame = frame.read().unwrap();
            let (_, shorts, _) = unsafe { g_frame.align_to::<u32>() };
            window.update_with_buffer(shorts, WIDTH, HEIGHT)?;
            drop(g_frame);
        }

        thread::sleep(Duration::from_millis(1000 / 30));
    }
}
