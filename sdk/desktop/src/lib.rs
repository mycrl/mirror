use std::{
    ffi::{c_char, c_void, CStr},
    ptr::null_mut,
    sync::Arc,
};

use anyhow::anyhow;
use bytes::Bytes;
use codec::video::{VideoEncoderSettings, VideoFrameReceiverProcesser, VideoFrameSenderProcesser};
use common::frame::VideoFrame;
use devices::{Device, DeviceKind, DeviceManagerOptions, VideoInfo, VideoSink};
use once_cell::sync::Lazy;
use tokio::runtime;
use transport::{
    adapter::{StreamBufferInfo, StreamKind, StreamReceiverAdapter, StreamSenderAdapter},
    Transport,
};

static RUNTIME: Lazy<runtime::Runtime> = Lazy::new(|| {
    runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect(
            "Unable to initialize the internal asynchronous runtime, this is a very serious error.",
        )
});

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

#[no_mangle]
extern "C" fn init(options: RawDeviceOptions) -> bool {
    log::info!("create device manager: options={:?}", options);

    #[cfg(debug_assertions)]
    {
        simple_logger::init_with_level(log::Level::Info).expect("Failed to create logger.");
    }

    let func = || {
        {
            let mut path = std::env::current_exe()?;
            path.pop();
            std::env::set_current_dir(path)?;
        }

        Ok::<(), anyhow::Error>(devices::init(DeviceManagerOptions {
            video: VideoInfo {
                fps: options.fps,
                width: options.width,
                height: options.height,
            },
        })?)
    };

    func().is_ok()
}

#[no_mangle]
extern "C" fn quit() {
    log::info!("close device manager");

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
extern "C" fn create_mirror(multicast: *const c_char) -> *const RawMirror {
    assert!(!multicast.is_null());

    let func = || {
        let multicast = unsafe { CStr::from_ptr(multicast) }.to_str()?.parse()?;

        log::info!("create mirror: multicast={}", multicast);

        Ok::<RawMirror, anyhow::Error>(RawMirror {
            transport: RUNTIME.block_on(Transport::new::<()>(multicast, None))?,
        })
    };

    func()
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
    video_encoder: VideoFrameSenderProcesser,
    adapter: Arc<StreamSenderAdapter>,
}

impl SenderObserver {
    fn new(
        options: RawVideoEncoderOptions,
        adapter: Arc<StreamSenderAdapter>,
    ) -> anyhow::Result<Self> {
        Ok(Self {
            adapter,
            video_encoder: VideoFrameSenderProcesser::new(&VideoEncoderSettings {
                codec_name: unsafe { CStr::from_ptr(options.codec_name) }
                    .to_str()?
                    .to_string(),
                width: options.width,
                height: options.height,
                bit_rate: options.bit_rate,
                frame_rate: options.frame_rate,
                max_b_frames: options.max_b_frames,
                key_frame_interval: options.key_frame_interval,
            })
            .ok_or_else(|| anyhow!("Failed to create video encoder."))?,
        })
    }
}

impl VideoSink for SenderObserver {
    fn sink(&self, frame: &VideoFrame) {
        if self.video_encoder.push_frame(frame) {
            while let Some(packet) = self.video_encoder.read_packet() {
                self.adapter.send(
                    Bytes::copy_from_slice(packet.buffer),
                    StreamBufferInfo::Video(packet.flags),
                );
            }
        }
    }
}

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

#[no_mangle]
extern "C" fn create_sender(
    mirror: *const RawMirror,
    mtu: usize,
    bind: *const c_char,
    codec_options: RawVideoEncoderOptions,
) -> bool {
    assert!(!mirror.is_null());
    assert!(!bind.is_null());

    let func = || {
        let adapter = StreamSenderAdapter::new();
        let bind = unsafe { CStr::from_ptr(bind) }.to_str()?.parse()?;
        let mirror = unsafe { &*mirror };

        log::info!("create sender: mtu={}, bind={}", mtu, bind);

        RUNTIME.block_on(
            mirror
                .transport
                .create_sender(0, mtu, bind, Vec::new(), &adapter),
        )?;

        devices::set_video_sink(SenderObserver::new(codec_options, adapter)?);
        Ok::<(), anyhow::Error>(())
    };

    func().is_ok()
}

#[no_mangle]
extern "C" fn create_receiver(
    mirror: *const RawMirror,
    bind: *const c_char,
    codec: *const c_char,
    frame_proc: extern "C" fn(context: *const c_void, frame: *const VideoFrame) -> bool,
    context: *const c_void,
) -> bool {
    assert!(!mirror.is_null());
    assert!(!bind.is_null());

    let func = || {
        let adapter = StreamReceiverAdapter::new();
        let bind = unsafe { CStr::from_ptr(bind) }.to_str()?.parse()?;
        let codec = unsafe { CStr::from_ptr(codec) }.to_str()?;

        log::info!("create receiver: codec={}, bind={}", codec, bind);

        RUNTIME.block_on(
            unsafe { &*mirror }
                .transport
                .create_receiver(bind, &adapter),
        )?;

        let decoder = VideoFrameReceiverProcesser::new(codec)
            .ok_or_else(|| anyhow!("Failed to create video decoder."))?;

        let context = context as usize;
        RUNTIME.spawn(async move {
            'a: while let Some((packet, kind)) = adapter.next().await {
                if kind == StreamKind::Video {
                    if !decoder.push_packet(&packet) {
                        break;
                    }

                    while let Some(frame) = decoder.read_frame() {
                        if !frame_proc(context as *const _, frame) {
                            break 'a;
                        }
                    }
                }
            }
        });

        Ok::<(), anyhow::Error>(())
    };

    func().is_ok()
}
