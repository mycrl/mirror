use std::{ffi::c_void, sync::Arc};

use frame::{VideoFrameRect, VideoFrame};

use crate::{api, device::Device, DeviceError, DeviceKind, VideoInfo};

#[derive(Debug, Clone)]
pub struct DeviceManagerOptions {
    pub video: VideoInfo,
}

pub struct DeviceManager {
    opt: DeviceManagerOptions,
    ptr: api::DeviceManager,
    ctx: *const Context,
}

impl DeviceManager {
    pub fn new<O: Observer + 'static>(
        opt: DeviceManagerOptions,
        observer: O,
    ) -> Result<Self, DeviceError> {
        if unsafe { api::_init(&opt.video) } != 0 {
            return Err(DeviceError::InitializeFailed);
        }

        let ctx = Box::into_raw(Box::new(Context(Arc::new(observer)))) as *const _;
        unsafe {
            api::_set_video_output_callback(
                video_sink_proc,
                VideoFrameRect {
                    width: opt.video.width as usize,
                    height: opt.video.height as usize,
                },
                ctx as *const c_void,
            );
        }

        let ptr = unsafe { api::_create_device_manager() };
        if ptr.is_null() {
            Err(DeviceError::CreateDeviceManagerFailed)
        } else {
            Ok(Self { ptr, ctx, opt })
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
        drop(unsafe { Box::from_raw(self.ctx as *mut Context) })
    }
}

pub trait Observer {
    fn video_sink(&self, frmae: &VideoFrame);
}

struct Context(Arc<dyn Observer>);

extern "C" fn video_sink_proc(ctx: *const c_void, frame: VideoFrame) {
    unsafe { &*(ctx as *const Context) }.0.video_sink(&frame);
}
