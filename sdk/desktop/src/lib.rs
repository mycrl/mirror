mod mirror;

use std::{
    ffi::{c_char, c_void},
    fmt::Debug,
    ptr::null_mut,
    sync::Arc,
};

use common::{frame::VideoFrame, strings::Strings};
use devices::{Device, DeviceKind, DeviceManager};
use mirror::{Mirror, MirrorOptions, VideoOptions};
use transport::adapter::{StreamReceiverAdapter, StreamSenderAdapter};

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct RawVideoOptions {
    encoder: *const c_char,
    decoder: *const c_char,
    max_b_frames: u8,
    frame_rate: u8,
    width: u32,
    height: u32,
    bit_rate: u64,
    key_frame_interval: u32,
}

unsafe impl Send for RawVideoOptions {}
unsafe impl Sync for RawVideoOptions {}

impl TryInto<VideoOptions> for RawVideoOptions {
    type Error = anyhow::Error;

    fn try_into(self) -> Result<VideoOptions, Self::Error> {
        Ok(VideoOptions {
            encoder: Strings::from(self.encoder).to_string()?,
            decoder: Strings::from(self.decoder).to_string()?,
            key_frame_interval: self.key_frame_interval,
            max_b_frames: self.max_b_frames,
            frame_rate: self.frame_rate,
            width: self.width,
            height: self.height,
            bit_rate: self.bit_rate,
        })
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct RawMirrorOptions {
    video: RawVideoOptions,
    multicast: *const c_char,
    mtu: usize,
}

unsafe impl Send for RawMirrorOptions {}
unsafe impl Sync for RawMirrorOptions {}

impl TryInto<MirrorOptions> for RawMirrorOptions {
    type Error = anyhow::Error;

    fn try_into(self) -> Result<MirrorOptions, Self::Error> {
        Ok(MirrorOptions {
            multicast: Strings::from(self.multicast).to_string()?,
            video: self.video.try_into()?,
            mtu: self.mtu,
        })
    }
}

#[no_mangle]
extern "C" fn init(options: RawMirrorOptions) -> bool {
    checker((|| mirror::init(options.try_into()?))()).is_ok()
}

#[no_mangle]
extern "C" fn quit() {
    mirror::quit()
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

#[repr(C)]
pub struct RawDevices {
    pub list: *const Device,
    pub capacity: usize,
    pub size: usize,
}

#[no_mangle]
extern "C" fn get_devices(kind: DeviceKind) -> RawDevices {
    log::info!("get devices: kind={:?}", kind);

    let devices = DeviceManager::get_devices(kind).to_vec();
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
extern "C" fn set_input_device(device: *const Device) {
    assert!(!device.is_null());

    mirror::set_input_device(unsafe { &*device });
}

#[repr(C)]
pub struct RawMirror {
    mirror: Mirror,
}

#[no_mangle]
extern "C" fn create_mirror() -> *const RawMirror {
    checker(Mirror::new())
        .map(|mirror| Box::into_raw(Box::new(RawMirror { mirror })))
        .unwrap_or_else(|_| null_mut()) as *const _
}

#[no_mangle]
extern "C" fn drop_mirror(mirror: *const RawMirror) {
    assert!(!mirror.is_null());

    drop(unsafe { Box::from_raw(mirror as *mut RawMirror) });

    log::info!("close mirror");
}

#[repr(C)]
pub struct RawSender {
    adapter: Arc<StreamSenderAdapter>,
}

#[no_mangle]
extern "C" fn create_sender(
    mirror: *const RawMirror,
    bind: *const c_char,
    callback: Option<extern "C" fn(ctx: *const c_void, frame: *const VideoFrame) -> bool>,
    ctx: *const c_void,
) -> *const RawSender {
    assert!(!mirror.is_null());
    assert!(!bind.is_null());

    let ctx = ctx as usize;
    checker((|| {
        unsafe { &*mirror }.mirror.create_sender(
            &Strings::from(bind).to_string()?,
            callback.map(|callback| move |frame: &VideoFrame| callback(ctx as *const _, frame)),
        )
    })())
    .map(|adapter| Box::into_raw(Box::new(RawSender { adapter })))
    .unwrap_or_else(|_| null_mut())
}

#[no_mangle]
extern "C" fn close_sender(sender: *const RawSender) {
    assert!(!sender.is_null());

    drop(unsafe { Box::from_raw(sender as *mut RawSender) });

    log::info!("close sender");
}

#[repr(C)]
pub struct RawReceiver {
    adapter: Arc<StreamReceiverAdapter>,
}

#[no_mangle]
extern "C" fn create_receiver(
    mirror: *const RawMirror,
    bind: *const c_char,
    callback: extern "C" fn(ctx: *const c_void, frame: *const VideoFrame) -> bool,
    ctx: *const c_void,
) -> *const RawReceiver {
    assert!(!mirror.is_null());
    assert!(!bind.is_null());

    let ctx = ctx as usize;
    checker((|| {
        unsafe { &*mirror }
            .mirror
            .create_receiver(&Strings::from(bind).to_string()?, move |frame| {
                callback(ctx as *const _, frame)
            })
    })())
    .map(|adapter| Box::into_raw(Box::new(RawReceiver { adapter })))
    .unwrap_or_else(|_| null_mut())
}

#[no_mangle]
extern "C" fn close_receiver(receiver: *const RawReceiver) {
    assert!(!receiver.is_null());

    drop(unsafe { Box::from_raw(receiver as *mut RawReceiver) });

    log::info!("close receiver");
}

#[inline]
fn checker<T, E: Debug>(result: Result<T, E>) -> Result<T, E> {
    if let Err(e) = &result {
        log::error!("{:?}", e);
    }

    result
}
