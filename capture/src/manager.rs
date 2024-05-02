use crate::device::{Device, DeviceKind, DeviceList, RawDeviceDescription, RawDeviceList};

extern "C" {
    /// Enumerates all input sources.
    ///
    /// Callback function returns true to continue enumeration, or false to
    /// end enumeration.
    pub fn capture_get_device_list(kind: DeviceKind) -> *const RawDeviceList;
    /// Sets the primary output source for a channel.
    pub fn capture_set_video_input(description: *const RawDeviceDescription);
}

pub struct DeviceManager;

impl DeviceManager {
    /// To get a list of devices, you need to specify the type of device to get.
    ///
    /// ```
    /// let devices = get_devices(DeviceKind::Video).to_vec();
    /// ```
    pub fn get_devices(kind: DeviceKind) -> DeviceList {
        log::info!("DeviceManager get devices");

        DeviceList(unsafe { capture_get_device_list(kind) })
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
    pub fn set_input(device: &Device) {
        log::info!("DeviceManager set input device");

        unsafe { capture_set_video_input(device.as_ptr()) }
    }
}
