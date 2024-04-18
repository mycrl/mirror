use std::{
    ffi::{c_char, c_void},
    fmt::Debug,
    ptr::null_mut,
    sync::{Arc, RwLock},
};

use anyhow::anyhow;
use bytes::Bytes;
use codec::{VideoDecoder, VideoEncoder, VideoEncoderSettings};
use common::{
    frame::VideoFrame,
    strings::{StringError, Strings},
};
use devices::{Device, DeviceKind, DeviceManagerOptions, VideoInfo, VideoSink};
use once_cell::sync::Lazy;
use tokio::runtime;
use transport::{
    adapter::{StreamBufferInfo, StreamKind, StreamReceiverAdapter, StreamSenderAdapter},
    Transport,
};

static OPTIONS: Lazy<RwLock<Option<RawMirrorOptions>>> = Lazy::new(|| Default::default());
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

impl TryInto<VideoEncoderSettings> for RawVideoOptions {
    type Error = StringError;

    fn try_into(self) -> Result<VideoEncoderSettings, Self::Error> {
        Ok(VideoEncoderSettings {
            width: self.width,
            height: self.height,
            bit_rate: self.bit_rate,
            frame_rate: self.frame_rate,
            max_b_frames: self.max_b_frames,
            key_frame_interval: self.key_frame_interval,
            codec_name: Strings::from(self.encoder).to_string()?,
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

#[no_mangle]
extern "C" fn init(options: RawMirrorOptions) -> bool {
    #[cfg(debug_assertions)]
    {
        simple_logger::init_with_level(log::Level::Debug).expect("Failed to create logger.");
    }

    let _ = OPTIONS.write().unwrap().replace(options);
    let func = || {
        {
            let mut path = std::env::current_exe()?;
            path.pop();
            std::env::set_current_dir(path)?;
        }

        Ok::<(), anyhow::Error>(devices::init(DeviceManagerOptions {
            video: VideoInfo {
                width: options.video.width,
                height: options.video.height,
                fps: options.video.frame_rate,
            },
        })?)
    };

    log::info!("mirror init: options={:?}", options);

    checker(func()).is_ok()
}

#[no_mangle]
extern "C" fn quit() {
    log::info!("close mirror");

    devices::quit();
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
    let devices = devices::get_devices(kind);
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

    let device = unsafe { &*device };
    devices::set_input(device);

    log::info!("set input to device manager: device={:?}", device.name());
}

#[repr(C)]
pub struct RawMirror {
    transport: Transport,
}

#[no_mangle]
extern "C" fn create_mirror() -> *const RawMirror {
    let options = OPTIONS.read().unwrap().expect("Not initialized yet!");
    let func = || {
        let multicast = Strings::from(options.multicast).to_string()?.parse()?;

        log::info!("create mirror: multicast={}", multicast);

        Ok::<RawMirror, anyhow::Error>(RawMirror {
            transport: RUNTIME.block_on(Transport::new::<()>(multicast, None))?,
        })
    };

    checker(func())
        .map(|it| Box::into_raw(Box::new(it)))
        .unwrap_or_else(|_| null_mut()) as *const _
}

#[no_mangle]
extern "C" fn drop_mirror(mirror: *const RawMirror) {
    assert!(!mirror.is_null());

    log::info!("close mirror");

    drop(unsafe { Box::from_raw(mirror as *mut RawMirror) })
}

struct SenderObserver {
    video_encoder: VideoEncoder,
    adapter: Arc<StreamSenderAdapter>,
}

impl SenderObserver {
    fn new(adapter: Arc<StreamSenderAdapter>) -> anyhow::Result<Self> {
        let options = OPTIONS.read().unwrap().expect("Not initialized yet!");
        Ok(Self {
            adapter,
            video_encoder: VideoEncoder::new(&options.video.try_into()?)
                .ok_or_else(|| anyhow!("Failed to create video encoder."))?,
        })
    }
}

impl VideoSink for SenderObserver {
    fn sink(&self, frame: &VideoFrame) {
        if self.video_encoder.encode(frame) {
            while let Some(packet) = self.video_encoder.read() {
                self.adapter.send(
                    Bytes::copy_from_slice(packet.buffer),
                    StreamBufferInfo::Video(packet.flags),
                );
            }
        }
    }
}

#[no_mangle]
extern "C" fn create_sender(mirror: *const RawMirror, bind: *const c_char) -> bool {
    assert!(!mirror.is_null());
    assert!(!bind.is_null());

    let options = OPTIONS.read().unwrap().expect("Not initialized yet!");
    let func = || {
        let adapter = StreamSenderAdapter::new();
        let bind = Strings::from(bind).to_string()?.parse()?;
        let mirror = unsafe { &*mirror };

        log::info!("create sender: bind={}", bind);

        RUNTIME.block_on(mirror.transport.create_sender(
            0,
            options.mtu,
            bind,
            Vec::new(),
            &adapter,
        ))?;

        devices::set_video_sink(SenderObserver::new(adapter)?);
        Ok::<(), anyhow::Error>(())
    };

    checker(func()).is_ok()
}

#[no_mangle]
extern "C" fn create_receiver(
    mirror: *const RawMirror,
    bind: *const c_char,
    frame_proc: extern "C" fn(context: *const c_void, frame: *const VideoFrame) -> bool,
    context: *const c_void,
) -> bool {
    assert!(!mirror.is_null());
    assert!(!bind.is_null());

    let options = OPTIONS.read().unwrap().expect("Not initialized yet!");
    let func = || {
        let adapter = StreamReceiverAdapter::new();
        let codec = Strings::from(options.video.decoder).to_string()?;
        let bind = Strings::from(bind).to_string()?.parse()?;

        log::info!("create receiver: bind={}", bind);

        RUNTIME.block_on(
            unsafe { &*mirror }
                .transport
                .create_receiver(bind, &adapter),
        )?;

        let video_decoder =
            VideoDecoder::new(&codec).ok_or_else(|| anyhow!("Failed to create video decoder."))?;

        let context = context as usize;
        RUNTIME.spawn(async move {
            'a: while let Some((packet, kind)) = adapter.next().await {
                if kind == StreamKind::Video {
                    if !video_decoder.decode(&packet) {
                        break;
                    }

                    while let Some(frame) = video_decoder.read() {
                        if !frame_proc(context as *const _, frame) {
                            break 'a;
                        }
                    }
                }
            }
        });

        Ok::<(), anyhow::Error>(())
    };

    checker(func()).is_ok()
}

#[inline]
fn checker<T, E: Debug>(result: Result<T, E>) -> Result<T, E> {
    if let Err(e) = &result {
        log::error!("{:?}", e);
    }

    result
}
