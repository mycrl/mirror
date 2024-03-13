use std::ffi::c_char;

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
    pub items: *const Device,
    pub size: usize,
}

impl Drop for Devices {
    fn drop(&mut self) {
        unsafe { release_devices(self) }
    }
}

extern "C" {
    pub fn init();
    pub fn get_audio_devices() -> Devices;
    pub fn get_video_devices() -> Devices;
    pub fn release_devices(devices: *const Devices);
}
