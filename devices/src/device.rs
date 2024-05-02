use std::ffi::{c_char, c_int};

use common::strings::Strings;

#[repr(C)]
pub struct RawDeviceDescription {
    pub kind: DeviceKind,
    pub id: *const c_char,
    pub name: *const c_char,
    pub index: c_int,
}

#[repr(C)]
pub struct RawDeviceList {
    pub devices: *const *const RawDeviceDescription,
    pub size: usize,
}

extern "C" {
    pub fn devices_release_device_list(list: *const RawDeviceList);
    pub fn devices_release_device_description(description: *const RawDeviceDescription);
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeviceKind {
    Video,
    Audio,
    Screen,
}

/// Indicates the system device captured by OBS, which can be a screen, camera,
/// window, etc.
#[derive(Debug)]
pub struct Device(*const RawDeviceDescription);

impl Device {
    #[inline]
    pub(crate) fn as_ptr(&self) -> *const RawDeviceDescription {
        self.0
    }

    /// Get the name of the device, as the device name may not be in the correct
    /// UTF8 encoding and there may be no name.
    #[inline]
    pub fn name(&self) -> Option<String> {
        Strings::from(unsafe { &*self.0 }.name).to_string().ok()
    }

    /// Get the ID of the device, which usually consists of the device UID plus
    /// the device name, because the device ID may not be the correct UTF8
    /// encoding and there may be no ID.
    #[inline]
    pub fn id(&self) -> Option<String> {
        Strings::from(unsafe { &*self.0 }.id).to_string().ok()
    }

    /// For the case where there is no device name, it doesn't mean that the·
    /// device really doesn't have a name, but the name is not UTF8 encoded, and
    /// you can use this function to get the char pointer directly.
    #[inline]
    pub fn c_name(&self) -> *const c_char {
        unsafe { &*self.0 }.name
    }

    /// For the case where there is no device ID, it doesn't mean that this
    /// device really doesn't have an ID, but the ID is not UTF8 encoded, and
    /// you can use this function to get the char pointer directly.
    #[inline]
    pub fn c_id(&self) -> *const c_char {
        unsafe { &*self.0 }.id
    }

    #[inline]
    pub fn kind(&self) -> DeviceKind {
        unsafe { &*self.0 }.kind
    }
}

impl Drop for Device {
    fn drop(&mut self) {
        unsafe { devices_release_device_description(self.0) }
    }
}

/// Underlying reference to a list of storage devices that can be directly
/// converted to a Vec.
pub struct DeviceList(pub(crate) *const RawDeviceList);

impl DeviceList {
    /// Converts the underlying reference to a Vec<Device>.
    ///
    /// ```
    /// get_devices(DeviceKind::Video).to_vec();
    /// ```
    pub fn to_vec(&self) -> Vec<Device> {
        let list = unsafe { &*self.0 };
        unsafe { std::slice::from_raw_parts(list.devices, list.size) }
            .into_iter()
            .map(|item| Device(*item))
            .collect()
    }
}

impl Drop for DeviceList {
    fn drop(&mut self) {
        unsafe { devices_release_device_list(self.0) }
    }
}
