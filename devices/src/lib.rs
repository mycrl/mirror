mod api;

pub use api::DeviceConstraint;

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

pub struct Device(*const api::DeviceInfo);

impl Drop for Device {
    fn drop(&mut self) {
        unsafe { api::release_device_info(self.0) }
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

    pub fn open(&self, constraint: DeviceConstraint) -> Result<DeviceManager, DeviceError> {
        let device = unsafe { api::open_device(self.0, constraint) };
        if !device.is_null() {
            Ok(DeviceManager::new(device))
        } else {
            Err(DeviceError::InvalidDevice)
        }
    }
}

#[derive(Debug)]
pub struct VideoFrame<'a> {
    pub format: u32,
    pub width: u32,
    pub height: u32,
    pub planes: Vec<&'a [u8]>,
}

pub struct DeviceManager {
    ptr: *const api::Device,
}

impl Drop for DeviceManager {
    fn drop(&mut self) {
        unsafe { api::release_device(self.ptr) }
    }
}

impl DeviceManager {
    fn new(ptr: *const api::Device) -> Self {
        Self { ptr }
    }

    pub fn make_readable(&self) -> bool {
        let mut ret = unsafe { api::device_advance(self.ptr) };
        if ret == -2 {
            ret = unsafe { api::device_advance(self.ptr) };
        }

        ret == 0
    }

    pub fn get_frame(&self) -> Option<VideoFrame> {
        let frame = unsafe { api::device_get_frame(self.ptr) };
        if !frame.is_null() {
            let frame = unsafe { &*frame };
            let mut planes = Vec::with_capacity(8);

            let sizes = unsafe { std::slice::from_raw_parts(frame.linesizes, 8) };
            for (i, plane) in unsafe { std::slice::from_raw_parts(frame.planes, 8) }
                .iter()
                .enumerate()
            {
                if sizes[i] > 0 {
                    planes.push(unsafe { std::slice::from_raw_parts(*plane, sizes[i] as usize) });
                }
            }

            Some(VideoFrame {
                format: frame.format as u32,
                width: frame.width,
                height: frame.height,
                planes,
            })
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
