use std::{
    ffi::{c_char, c_void, CStr},
    ptr::null,
};

use api::get_device_name;

mod api {
    use std::ffi::{c_char, c_void};

    extern "C" {
        pub fn init();
        pub fn get_audio_device_next(device: *const c_void) -> *const c_void;
        pub fn get_video_device_next(device: *const c_void) -> *const c_void;
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
struct Device {
    ptr: *const c_void,
    pub name: Option<String>,
    pub kind: DeviceKind,
}

struct Devices;

impl Devices {
    fn get_audio_devices() -> Vec<Device> {
        let mut items = Vec::with_capacity(20);

        let mut ptr = null();
        loop {
            ptr = unsafe { api::get_audio_device_next(ptr) };
            if !ptr.is_null() {
                items.push(Device {
                    name: from_c_str(unsafe { get_device_name(ptr) }),
                    kind: DeviceKind::Audio,
                    ptr,
                })
            } else {
                break;
            }
        }

        items
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
