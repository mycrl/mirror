use crate::{api, device::Device, DeviceError, DeviceKind, VideoInfo};

#[derive(Debug, Clone)]
pub struct DeviceManagerOptions {
    pub video: VideoInfo,
}

pub struct DeviceManager {
    opt: DeviceManagerOptions,
    ptr: api::DeviceManager,
}

impl DeviceManager {
    pub fn new(opt: DeviceManagerOptions) -> Result<Self, DeviceError> {
        if unsafe { api::_init(&opt.video) } != 0 {
            return Err(DeviceError::InitializeFailed);
        }

        let ptr = unsafe { api::_create_device_manager() };
        if ptr.is_null() {
            Err(DeviceError::CreateDeviceManagerFailed)
        } else {
            Ok(Self { ptr, opt })
        }
    }

    pub fn get_devices(&self, kind: DeviceKind) -> Vec<Device> {
        let list = unsafe { api::_get_device_list(self.ptr, kind) };
        unsafe { std::slice::from_raw_parts(list.devices, list.size) }
            .into_iter()
            .map(|item| Device::new(*item))
            .collect()
    }

    pub fn set_input(&self, device: &Device) {
        if device.kind() == DeviceKind::Video {
            unsafe { api::_set_video_input(self.ptr, device.as_ptr(), &self.opt.video) }
        }
    }
}

impl Drop for DeviceManager {
    fn drop(&mut self) {
        unsafe { api::_device_manager_release(self.ptr) }
    }
}
