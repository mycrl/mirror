use std::{ffi::c_void, sync::Arc};

use crate::{
    api::{self, VideoFrame},
    device::Device,
    DeviceError, DeviceKind, Frame, VideoInfo,
};

#[derive(Debug, Clone)]
pub struct DeviceManagerOptions {
    pub video: VideoInfo,
}

pub struct DeviceManager {
    ptr: api::DeviceManager,
    ctx: *const Context,
}

impl DeviceManager {
    pub fn new<O: Observer + 'static>(
        opt: DeviceManagerOptions,
        observer: O,
    ) -> Result<Self, DeviceError> {
        if unsafe { api::init(&opt.video) } != 0 {
            return Err(DeviceError::InitializeFailed);
        }

        let ctx = Box::into_raw(Box::new(Context {
            observer: Arc::new(observer),
            opt: opt.clone(),
        })) as *const _;

        unsafe {
            api::set_video_output_callback(video_sink_proc, ctx as *const c_void);
        }

        let ptr = unsafe { api::create_device_manager(&opt.video) };
        if ptr.is_null() {
            Err(DeviceError::CreateDeviceManagerFailed)
        } else {
            Ok(Self { ptr, ctx })
        }
    }

    pub fn get_devices(&self, kind: DeviceKind) -> Vec<Device> {
        let list = unsafe { api::get_device_list(self.ptr, kind) };
        unsafe { std::slice::from_raw_parts(list.devices, list.size) }
            .into_iter()
            .map(|item| Device::new(*item))
            .collect()
    }

    pub fn set_input(&self, device: &Device) {
        if device.kind() == DeviceKind::Video {
            unsafe { api::set_video_input(self.ptr, device.as_ptr()) }
        }
    }
}

impl Drop for DeviceManager {
    fn drop(&mut self) {
        unsafe { api::device_manager_release(self.ptr) }
        drop(unsafe { Box::from_raw(self.ctx as *mut Context) })
    }
}

pub trait Observer {
    fn video_sink(&self, frmae: Frame);
}

struct Context {
    observer: Arc<dyn Observer>,
    opt: DeviceManagerOptions,
}

extern "C" fn video_sink_proc(ctx: *const c_void, frame: *const VideoFrame) {
    let ctx = unsafe { &*(ctx as *const Context) };
    let frame = Frame::from_raw(frame, &ctx.opt.video);
    ctx.observer.video_sink(frame);
}
