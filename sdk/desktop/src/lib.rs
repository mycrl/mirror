use std::{
    ffi::{c_char, CStr},
    ptr::null_mut,
};

use devices::{DeviceManager, DeviceManagerOptions, VideoFormat, VideoInfo};
use once_cell::sync::Lazy;
use tokio::runtime;
use transport::Transport;

static RUNTIME: Lazy<runtime::Runtime> = Lazy::new(|| {
    runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect(
            "Unable to initialize the internal asynchronous runtime, this is a very serious error.",
        )
});

struct DeviceManagerObserver {}

impl devices::Observer for DeviceManagerObserver {
    fn video_sink(&self, frmae: devices::Frame) {
        
    }
}

#[repr(C)]
pub struct MirrorOptions {
    multicast: *const c_char,
    width: u32,
    height: u32,
    fps: u8,
}

pub struct Mirror {
    device_manager: DeviceManager,
    options: MirrorOptions,
    transport: Transport,
}

#[no_mangle]
extern "C" fn create_mirror(options: MirrorOptions) -> *const Mirror {
    let func = || {
        let multicast = unsafe { CStr::from_ptr(options.multicast) }
            .to_str()?
            .parse()?;

        Ok::<Mirror, anyhow::Error>(Mirror {
            transport: RUNTIME.block_on(Transport::new::<()>(multicast, None))?,
            device_manager: DeviceManager::new(
                DeviceManagerOptions {
                    video: VideoInfo {
                        fps: options.fps,
                        width: options.width,
                        height: options.height,
                        format: VideoFormat::VIDEO_FORMAT_NV12,
                    },
                },
                DeviceManagerObserver {},
            )?,
            options,
        })
    };

    func()
        .map(|mirror| Box::into_raw(Box::new(mirror)))
        .unwrap_or_else(|_| null_mut())
}

#[no_mangle]
extern "C" fn get_devices(mirror: *const Mirror) {

}
