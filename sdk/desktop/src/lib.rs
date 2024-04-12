use std::{
    ffi::{c_char, CStr},
    ptr::null_mut,
    sync::{Arc, RwLock},
};

use anyhow::anyhow;
use codec::video::{VideoEncoderSettings, VideoFrameSenderProcesser};
use devices::{Device, DeviceKind, DeviceManager, DeviceManagerOptions, VideoFormat, VideoInfo};
use once_cell::sync::Lazy;
use tokio::runtime;
use transport::{adapter::{StreamBufferInfo, StreamSenderAdapter}, Transport};

static RUNTIME: Lazy<runtime::Runtime> = Lazy::new(|| {
    runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect(
            "Unable to initialize the internal asynchronous runtime, this is a very serious error.",
        )
});

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct RawVideoEncoderOptions {
    codec_name: *const c_char,
    max_b_frames: u8,
    frame_rate: u8,
    width: u32,
    height: u32,
    bit_rate: u64,
    key_frame_interval: u32,
}

struct DeviceManagerObserver {
    video_encoder: VideoFrameSenderProcesser,
    adapter: Arc<RwLock<Option<Arc<StreamSenderAdapter>>>>,
}

impl DeviceManagerObserver {
    fn new(
        options: RawDeviceManagerOptions,
        adapter: Arc<RwLock<Option<Arc<StreamSenderAdapter>>>>,
    ) -> anyhow::Result<Self> {
        Ok(Self {
            adapter,
            video_encoder: VideoFrameSenderProcesser::new(&VideoEncoderSettings {
                codec_name: unsafe { CStr::from_ptr(options.video_encoder.codec_name) }
                    .to_str()?
                    .to_string(),
                width: options.video_encoder.width,
                height: options.video_encoder.height,
                bit_rate: options.video_encoder.bit_rate,
                frame_rate: options.video_encoder.frame_rate,
                max_b_frames: options.video_encoder.max_b_frames,
                key_frame_interval: options.video_encoder.key_frame_interval,
            })
            .ok_or_else(|| anyhow!("Failed to create video encoder."))?,
        })
    }
}

impl devices::Observer for DeviceManagerObserver {
    fn video_sink(&self, frmae: devices::Frame) {
        if let Some(adapter) = self.adapter.read().unwrap().as_ref() {
            for packet in self.video_encoder.encode(None) {
                adapter.send(packet, StreamBufferInfo::Video(0));
            }
        }
    }
}

#[repr(C)]
pub struct RawDevices {
    pub list: *const Device,
    pub capacity: usize,
    pub size: usize,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct RawDeviceOptions {
    fps: u8,
    width: u32,
    height: u32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct RawDeviceManagerOptions {
    device: RawDeviceOptions,
    video_encoder: RawVideoEncoderOptions,
}

#[repr(C)]
pub struct RawDeviceManager {
    device_manager: DeviceManager,
    adapter: Arc<RwLock<Option<Arc<StreamSenderAdapter>>>>,
}

#[no_mangle]
extern "C" fn create_device_manager(options: RawDeviceManagerOptions) -> *const RawDeviceManager {
    let func = || {
        let adapter = Arc::new(RwLock::new(None));

        Ok::<RawDeviceManager, anyhow::Error>(RawDeviceManager {
            device_manager: DeviceManager::new(
                DeviceManagerOptions {
                    video: VideoInfo {
                        fps: options.device.fps,
                        width: options.device.width,
                        height: options.device.height,
                        format: VideoFormat::VIDEO_FORMAT_NV12,
                    },
                },
                DeviceManagerObserver::new(options, adapter.clone())?,
            )?,
            adapter,
        })
    };

    func()
        .map(|it| Box::into_raw(Box::new(it)))
        .unwrap_or_else(|_| null_mut()) as *const _
}

#[no_mangle]
extern "C" fn drop_device_manager(raw: *const RawDeviceManager) {
    assert!(!raw.is_null());

    drop(unsafe { Box::from_raw(raw as *mut RawDeviceManager) })
}

#[no_mangle]
extern "C" fn get_device_name(device: *const Device) -> *const c_char {
    assert!(!device.is_null());

    unsafe { &*device }.c_name()
}

#[no_mangle]
extern "C" fn get_device_kind(device: *const Device) -> DeviceKind {
    assert!(!device.is_null());

    unsafe { &*device }.kind()
}

#[no_mangle]
extern "C" fn get_devices(raw: *const RawDeviceManager, kind: DeviceKind) -> RawDevices {
    assert!(!raw.is_null());

    let raw = unsafe { &*raw };
    let devices = raw.device_manager.get_devices(kind);
    let raw_devices = RawDevices {
        capacity: devices.capacity(),
        list: devices.as_ptr(),
        size: devices.len(),
    };

    std::mem::forget(devices);
    raw_devices
}

#[no_mangle]
extern "C" fn drop_devices(devices: *const RawDevices) {
    assert!(!devices.is_null());

    let devices = unsafe { &*devices };
    drop(unsafe { Vec::from_raw_parts(devices.list as *mut Device, devices.size, devices.size) })
}

#[no_mangle]
extern "C" fn set_input_device(raw: *const RawDeviceManager, device: *const Device) {
    assert!(!device.is_null());
    assert!(!raw.is_null());

    unsafe { &*raw }
        .device_manager
        .set_input(unsafe { &*device })
}

#[repr(C)]
pub struct RawMirror {
    transport: Transport,
}

#[no_mangle]
extern "C" fn create_mirrir(multicast: *const c_char) -> *const RawMirror {
    assert!(!multicast.is_null());

    let func = || {
        Ok::<RawMirror, anyhow::Error>(RawMirror {
            transport: RUNTIME.block_on(Transport::new::<()>(
                unsafe { CStr::from_ptr(multicast) }.to_str()?.parse()?,
                None,
            ))?,
        })
    };

    func()
        .map(|it| Box::into_raw(Box::new(it)))
        .unwrap_or_else(|_| null_mut()) as *const _
}

#[no_mangle]
extern "C" fn drop_mirror(mirror: *const RawMirror) {
    assert!(!mirror.is_null());

    drop(unsafe { Box::from_raw(mirror as *mut RawMirror) })
}

#[no_mangle]
extern "C" fn create_sender(
    mirror: *const RawMirror,
    device_manager: *const RawDeviceManager,
    mtu: usize,
    bind: *const c_char,
) -> bool {
    assert!(!device_manager.is_null());
    assert!(!mirror.is_null());
    assert!(!bind.is_null());

    let func = || {
        let adapter = StreamSenderAdapter::new();
        RUNTIME.block_on(unsafe { &*mirror }.transport.create_sender(
            0,
            mtu,
            unsafe { CStr::from_ptr(bind) }.to_str()?.parse()?,
            Vec::new(),
            &adapter,
        ))?;

        unsafe { &*device_manager }
            .adapter
            .write()
            .unwrap()
            .replace(adapter);
        Ok::<(), anyhow::Error>(())
    };

    func().is_ok()
}
