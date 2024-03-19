use std::ffi::{c_char, c_void};

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeviceKind {
    Video = 0,
    Audio = 1,
}

#[repr(C)]
pub struct Device {
    pub name: *const c_char,
    pub description: *const c_char,
    pub kind: DeviceKind,
}

#[repr(C)]
pub struct Devices {
    pub items: *const *const Device,
    pub size: usize,
}

impl Drop for Devices {
    fn drop(&mut self) {
        unsafe { release_devices(self) }
    }
}

#[repr(C)]
pub struct Buffer {
    pub data: *const u8,
    pub size: usize,
}

#[repr(C)]
pub struct Context {
    ctx: *const c_void,
    fmt: *const c_void,
    pkt: *const c_void,
    buf: *const Buffer,
}

extern "C" {
    pub fn init();
    pub fn get_audio_devices() -> Devices;
    pub fn get_video_devices() -> Devices;
    pub fn release_device(device: *const Device);
    pub fn release_devices(devices: *const Devices);
    pub fn open_device(device: *const Device) -> *const Context;
    pub fn release_device_context(ctx: *const Context);
    pub fn device_read_packet(ctx: *const Context) -> *const Buffer;
}
