use std::{
    ffi::{c_char, c_void},
    sync::Arc,
};

pub use api::{DeviceKind, VideoInfo};
use common::{frame::VideoFrame, strings::Strings};

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

extern "C" fn video_sink_proc(ctx: *const c_void, frame: *const VideoFrame) {
    unsafe { &*(ctx as *const Context) }
        .0
        .sink(unsafe { &*frame });
}

pub fn set_video_sink<S: VideoSink + 'static>(sink: S) {
    let previous = unsafe {
        api::_set_video_output_callback(
            video_sink_proc,
            Box::into_raw(Box::new(Context(Arc::new(sink)))) as *const c_void,
        )
    };

    if !previous.is_null() {
        drop(unsafe { Box::from_raw(previous as *mut Context) })
    }
}

pub struct Device {
    description: *const api::DeviceDescription,
}

impl Device {
    #[inline]
    pub(crate) fn new(description: *const api::DeviceDescription) -> Self {
        Self { description }
    }

    #[inline]
    pub(crate) fn as_ptr(&self) -> *const api::DeviceDescription {
        self.description
    }

    #[inline]
    pub fn name(&self) -> Option<String> {
        Strings::from(unsafe { &*self.description }.name)
            .to_string()
            .ok()
    }

    #[inline]
    pub fn c_name(&self) -> *const c_char {
        unsafe { &*self.description }.name
    }

    #[inline]
    pub fn kind(&self) -> DeviceKind {
        unsafe { &*self.description }.kind
    }
}

impl Drop for Device {
    fn drop(&mut self) {
        unsafe { api::_release_device_description(self.description) }
    }
}

#[derive(Debug, Clone)]
pub struct DeviceManagerOptions {
    pub video: VideoInfo,
}

pub fn init(options: DeviceManagerOptions) -> Result<(), DeviceError> {
    if unsafe { api::_init(&options.video) } != 0 {
        Err(DeviceError::InitializeFailed)
    } else {
        Ok(())
    }
}

pub fn quit() {
    unsafe { api::_quit() }
}

pub fn get_devices(kind: DeviceKind) -> Vec<Device> {
    log::info!("DeviceManager get devices");

    let list = unsafe { api::_get_device_list(kind) };
    unsafe { std::slice::from_raw_parts(list.devices, list.size) }
        .into_iter()
        .map(|item| Device::new(*item))
        .collect()
}

pub fn set_input(device: &Device) {
    log::info!("DeviceManager set input device");

    if device.kind() == DeviceKind::Video {
        unsafe { api::_set_video_input(device.as_ptr()) }
    }
}

mod api {
    use std::ffi::{c_char, c_int, c_void};

    use common::frame::VideoFrame;

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
        pub fn _quit();
        pub fn _init(info: *const VideoInfo) -> c_int;
        pub fn _get_device_list(kind: DeviceKind) -> DeviceList;
        pub fn _release_device_description(description: *const DeviceDescription);
        pub fn _set_video_input(description: *const DeviceDescription);
        pub fn _set_video_output_callback(
            proc: extern "C" fn(ctx: *const c_void, frame: *const VideoFrame),
            ctx: *const c_void,
        ) -> *const c_void;
    }
}
