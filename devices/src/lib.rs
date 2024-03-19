mod api;

use std::ffi::{c_char, CStr};

#[derive(Debug)]
pub enum DeviceError {
    InvalidDevice,
}

impl std::error::Error for DeviceError {}

impl std::fmt::Display for DeviceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                Self::InvalidDevice => "InvalidDevice",
            }
        )
    }
}

pub fn init() {
    unsafe { api::init() }
}

pub struct Device(*const api::Device);

impl Drop for Device {
    fn drop(&mut self) {
        unsafe { api::release_device(self.0) }
    }
}

impl Device {
    pub fn name(&self) -> Option<String> {
        from_c_str(unsafe { &*self.0 }.name)
    }

    pub fn description(&self) -> Option<String> {
        from_c_str(unsafe { &*self.0 }.description)
    }

    pub fn kind(&self) -> api::DeviceKind {
        unsafe { &*self.0 }.kind
    }

    pub fn open(&self) -> Result<DeviceManager, DeviceError> {
        let ctx = unsafe { api::open_device(self.0) };
        if !ctx.is_null() {
            Ok(DeviceManager(ctx))
        } else {
            Err(DeviceError::InvalidDevice)
        }
    }
}

pub struct DeviceManager(*const api::Context);

impl Drop for DeviceManager {
    fn drop(&mut self) {
        unsafe { api::release_device_context(self.0) }
    }
}

impl DeviceManager {
    pub fn next(&self) -> Option<&[u8]> {
        let pkt = unsafe { api::device_read_packet(self.0) };
        if !pkt.is_null() {
            Some(unsafe { std::slice::from_raw_parts((&*pkt).data, (&*pkt).size) })
        } else {
            None
        }
    }
}

pub struct Devices;

impl Devices {
    pub fn get_audio_devices() -> Vec<Device> {
        let list = unsafe { api::get_audio_devices() };
        unsafe { std::slice::from_raw_parts(list.items, list.size) }
            .into_iter()
            .map(|item| Device(*item))
            .collect()
    }

    pub fn get_video_devices() -> Vec<Device> {
        let list = unsafe { api::get_video_devices() };
        unsafe { std::slice::from_raw_parts(list.items, list.size) }
            .into_iter()
            .map(|item| Device(*item))
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
