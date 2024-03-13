mod api;

use std::ffi::{c_char, CStr};

pub fn init() {
    unsafe { api::init() }
}

#[derive(Debug)]
pub struct Device {
    pub kind: api::DeviceKind,
    pub name: Option<String>,
    pub description: Option<String>,
}

impl From<&api::Device> for Device {
    fn from(item: &api::Device) -> Self {
        Self {
            description: from_c_str(item.description),
            name: from_c_str(item.name),
            kind: item.kind,
        }
    }
}

pub struct Devices;

impl Devices {
    pub fn get_audio_devices() -> Vec<Device> {
        let list = unsafe { api::get_audio_devices() };
        unsafe { std::slice::from_raw_parts(list.items, list.size) }
            .into_iter()
            .map(|item| Device::from(item))
            .collect()
    }

    pub fn get_video_devices() -> Vec<Device> {
        let list = unsafe { api::get_video_devices() };
        unsafe { std::slice::from_raw_parts(list.items, list.size) }
            .into_iter()
            .map(|item| Device::from(item))
            .collect()
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
