use std::ffi::{c_char, c_int, c_void};

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeviceKind {
    Video = 0,
    Audio = 1,
}

#[repr(C)]
pub struct DeviceInfo {
    pub name: *const c_char,
    pub description: *const c_char,
    pub kind: DeviceKind,
    fmt: *const c_void,
}

#[repr(C)]
pub struct Devices {
    pub items: *const *const DeviceInfo,
    pub size: usize,
}

impl Drop for Devices {
    fn drop(&mut self) {
        unsafe { release_devices(self) }
    }
}

#[repr(C)]
pub struct VideoFrame {
    pub format: c_int,
    pub width: u32,
    pub height: u32,
    pub planes: *const *const u8,
    pub linesizes: *const u32,
}

pub type Device = c_void;

#[repr(C)]
pub struct DeviceConstraint {
    pub width: u32,
    pub height: u32,
    pub frame_rate: u8,
}

extern "C" {
    pub fn init();
    pub fn get_audio_devices() -> Devices;
    pub fn get_video_devices() -> Devices;
    pub fn release_device_info(device: *const DeviceInfo);
    pub fn release_devices(devices: *const Devices);
    pub fn open_device(device: *const DeviceInfo, constraint: DeviceConstraint) -> *const Device;
    pub fn release_device(device: *const Device);
    pub fn device_advance(device: *const Device) -> c_int;
    pub fn device_get_frame(device: *const Device) -> *const VideoFrame;
}
