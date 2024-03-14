mod api;

use std::ffi::{c_char, CStr, CString};

#[derive(Debug)]
pub enum DeviceError {
    InvalidDevice,
    FailedOpenDevice,
}

impl std::error::Error for DeviceError {}

impl std::fmt::Display for DeviceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                Self::InvalidDevice => "InvalidDevice",
                Self::FailedOpenDevice => "FailedOpenDevice",
            }
        )
    }
}

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

impl Device {
    pub fn open(&self) -> Result<DeviceManager, DeviceError> {
        let device = if let Some(name) = &self.name {
            to_c_str(&name)
        } else {
            return Err(DeviceError::InvalidDevice);
        };

        let ctx = unsafe { api::open_device(device) };
        release_c_str(device);

        if !ctx.is_null() {
            Ok(DeviceManager(ctx))
        } else {
            Err(DeviceError::FailedOpenDevice)
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

pub(crate) fn to_c_str(str: &str) -> *const c_char {
    CString::new(str).unwrap().into_raw()
}

pub(crate) fn release_c_str(str: *const c_char) {
    if !str.is_null() {
        drop(unsafe { CString::from_raw(str as *mut c_char) })
    }
}
