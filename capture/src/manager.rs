use std::{ffi::c_int, ptr::null};

use crate::{
    device::{Device, DeviceKind, DeviceList, RawDeviceDescription, RawDeviceList},
    CaptureSettings, DeviceError,
};

#[repr(C)]
struct RawGetDeviceListResult {
    status: c_int,
    list: *const RawDeviceList,
}

extern "C" {
    /// Enumerates all input sources.
    ///
    /// Callback function returns true to continue enumeration, or false to
    /// end enumeration.
    fn capture_get_device_list(
        kind: DeviceKind,
        settings: *const CaptureSettings,
    ) -> RawGetDeviceListResult;
    /// Sets the primary output source for a channel.
    fn capture_set_input(
        description: *const RawDeviceDescription,
        settings: *const CaptureSettings,
    ) -> c_int;
}

pub struct DeviceManager;

impl DeviceManager {
    /// To get a list of devices, you need to specify the type of device to get.
    ///
    /// ```
    /// let devices = get_devices(DeviceKind::Video).to_vec();
    /// ```
    pub fn get_devices(
        kind: DeviceKind,
        settings: Option<&CaptureSettings>,
    ) -> Result<DeviceList, DeviceError> {
        log::info!("DeviceManager get devices");

        let result = unsafe {
            capture_get_device_list(kind, if let Some(s) = settings { s } else { null() })
        };

        if result.status != 0 {
            Err(DeviceError(result.status))
        } else {
            Ok(DeviceList(result.list))
        }
    }

    /// Setting up an input device, it is important to note that a device of the
    /// same type will overwrite the previous device if it is set up repeatedly.
    ///
    /// ```
    /// let devices = get_devices(DeviceKind::Video).to_vec();
    /// for device in &devices {
    ///     println!("device: name={:?}, id={:?}", device.name(), device.id());
    /// }
    ///
    /// set_input(&devices[0]);
    /// ```
    pub fn set_input(
        device: &Device,
        settings: Option<&CaptureSettings>,
    ) -> Result<(), DeviceError> {
        log::info!("DeviceManager set input device");

        let status = unsafe {
            capture_set_input(
                device.as_ptr(),
                if let Some(s) = settings { s } else { null() },
            )
        };

        if status != 0 {
            Err(DeviceError(status))
        } else {
            Ok(())
        }
    }
}
