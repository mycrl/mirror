use std::ffi::{c_char, CStr};

use api::release_devices;

mod api {
    use std::ffi::{c_char, c_void};

    #[repr(C)]
    #[derive(Debug)]
    pub struct Devices {
        pub items: *const *const c_void,
        pub size: usize,
    }

    extern "C" {
        pub fn init();
        pub fn get_audio_devices() -> Devices;
        pub fn get_video_devices() -> Devices;
        pub fn release_devices(devices: *const Devices);
        pub fn get_device_name(device: *const c_void) -> *const c_char;
    }
}

fn main() {
    unsafe {
        api::init();
    }

    let devices = Devices::get_audio_devices();
    println!("{:?}", devices);
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DeviceKind {
    Video,
    Audio,
}

#[derive(Debug)]
struct Devices {
    raw: api::Devices,
    pub items: Vec<String>,
    pub kind: DeviceKind,
}

impl Devices {
    fn get_audio_devices() -> Self {
        let raw = unsafe { api::get_audio_devices() };
        let devices = unsafe { std::slice::from_raw_parts(raw.items, raw.size) };
        let mut items = Vec::with_capacity(devices.len());

        for device in devices {
            if let Some(name) = from_c_str(unsafe { api::get_device_name(*device) }) {
                items.push(name)
            }
        }

        Self {
            kind: DeviceKind::Audio,
            items,
            raw,
        }
    }
}

impl Drop for Devices {
    fn drop(&mut self) {
        unsafe { release_devices(&self.raw) }
    }
}

pub(crate) fn from_c_str(str: *const c_char) -> Option<String> {
    if !str.is_null() {
        unsafe { CStr::from_ptr(str) }
            .to_str()
            .map(|s| s.to_string())
            .ok()
    } else {
        None
    }
}
