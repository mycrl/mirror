mod device;
mod manager;
mod strings;

pub use api::{DeviceKind, VideoInfo};
pub use device::Device;
pub use manager::{DeviceManager, DeviceManagerOptions, Observer};

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

mod api {
    use std::ffi::{c_char, c_int, c_void};

    use frame::{FrameRect, VideoFrame};

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
            rect: FrameRect,
            ctx: *const c_void,
        );
    }
}
