use std::ffi::c_char;

use crate::{api, strings::Strings, DeviceKind};

pub struct Device {
    description: *const api::DeviceDescription,
}

impl Device {
    pub(crate) fn new(description: *const api::DeviceDescription) -> Self {
        Self { description }
    }

    pub(crate) fn as_ptr(&self) -> *const api::DeviceDescription {
        self.description
    }

    pub fn name(&self) -> Option<String> {
        Strings::from(unsafe { &*self.description }.name).to_string()
    }

    pub fn c_name(&self) -> *const c_char {
        unsafe { &*self.description }.name
    }

    pub fn kind(&self) -> DeviceKind {
        unsafe { &*self.description }.kind
    }
}

impl Drop for Device {
    fn drop(&mut self) {
        unsafe { api::_release_device_description(self.description) }
    }
}
