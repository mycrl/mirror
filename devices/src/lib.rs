mod device;
mod manager;

use std::{ffi::c_void, sync::Arc};

pub use api::{DeviceKind, VideoInfo};
use common::frame::{VideoFrame, VideoFrameRect};
pub use device::Device;
pub use manager::{DeviceManager, DeviceManagerOptions};

#[derive(Debug)]
pub enum DeviceError {
    InitializeFailed,
    CreateDeviceManagerFailed,
}

impl std::error::Error for DeviceError {}

impl std::fmt::Display for DeviceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                Self::CreateDeviceManagerFailed => "CreateDeviceManagerFailed",
                Self::InitializeFailed => "InitializeFailed",
            }
        )
    }
}

pub trait VideoSink {
    fn sink(&self, frmae: &VideoFrame);
}

struct Context(Arc<dyn VideoSink>);

extern "C" fn video_sink_proc(ctx: *const c_void, frame: VideoFrame) {
    unsafe { &*(ctx as *const Context) }.0.sink(&frame);
}

pub fn set_video_sink<S: VideoSink + 'static>(rect: VideoFrameRect, sink: S) {
    log::info!("set video sink for devices.");

    let previous = unsafe {
        api::_set_video_output_callback(
            video_sink_proc,
            rect,
            Box::into_raw(Box::new(Context(Arc::new(sink)))) as *const c_void,
        )
    };

    if !previous.is_null() {
        drop(unsafe { Box::from_raw(previous as *mut Context) })
    }
}

mod api {
    use std::ffi::{c_char, c_int, c_void};

    use common::frame::{VideoFrame, VideoFrameRect};

    pub type DeviceManager = *const c_void;

    #[repr(C)]
    #[derive(Debug, Clone)]
    pub struct VideoInfo {
        pub fps: u8,
        pub width: u32,
        pub height: u32,
    }

    #[repr(C)]
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum DeviceKind {
        Video,
        Audio,
        Screen,
    }

    #[repr(C)]
    pub struct DeviceDescription {
        pub kind: DeviceKind,
        pub id: *const c_char,
        pub name: *const c_char,
    }

    #[repr(C)]
    pub struct DeviceList {
        pub devices: *const *const DeviceDescription,
        pub size: usize,
    }

    extern "C" {
        pub fn _init(info: *const VideoInfo) -> c_int;
        pub fn _create_device_manager() -> DeviceManager;
        pub fn _device_manager_release(manager: DeviceManager);
        pub fn _get_device_list(manager: DeviceManager, kind: DeviceKind) -> DeviceList;
        pub fn _release_device_description(description: *const DeviceDescription);
        pub fn _set_video_input(
            manager: DeviceManager,
            description: *const DeviceDescription,
            info: *const VideoInfo,
        );

        pub fn _set_video_output_callback(
            proc: extern "C" fn(ctx: *const c_void, frame: VideoFrame),
            rect: VideoFrameRect,
            ctx: *const c_void,
        ) -> *const c_void;
    }
}
