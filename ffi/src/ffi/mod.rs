mod capture;
mod discovery;
mod observer;
mod renderer;

use std::{ffi::c_char, fmt::Debug, net::SocketAddr, ptr::null_mut};

use self::{capture::RawSource, observer::RawAVFrameStream};

use hylarana::{
    shutdown, startup, AudioDescriptor, GraphicsBackend, Hylarana, HylaranaReceiver,
    HylaranaReceiverDescriptor, HylaranaSender, HylaranaSenderDescriptor,
    HylaranaSenderSourceDescriptor, TransportDescriptor, TransportStrategy, VideoDecoderType,
    VideoDescriptor, VideoEncoderType,
};

use hylarana_common::{
    logger,
    strings::{write_c_str, Strings},
};

// In fact, this is a package that is convenient for recording errors. If the
// result is an error message, it is output to the log. This function does not
// make any changes to the result.
#[inline]
fn log_error<T, E: Debug>(result: Result<T, E>) -> Result<T, E> {
    if let Err(e) = &result {
        log::error!("{:?}", e);
    }

    result
}

/// Windows yes! The Windows dynamic library has an entry, so just
/// initialize the logger and set the process priority at the entry.
#[no_mangle]
#[allow(non_snake_case)]
#[cfg(target_os = "windows")]
extern "system" fn DllMain(
    _module: u32,
    call_reason: usize,
    reserved: *const std::ffi::c_void,
) -> bool {
    match call_reason {
        1 /* DLL_PROCESS_ATTACH */ => hylarana_startup(),
        0 /* DLL_PROCESS_DETACH */ => {
            if reserved.is_null() {
                hylarana_shutdown();
            }

            true
        },
        _ => true,
    }
}

/// Initialize the environment, which must be initialized before using the
/// SDK.
#[no_mangle]
extern "C" fn hylarana_startup() -> bool {
    let func = || {
        logger::init_logger(log::LevelFilter::Info, None)?;

        startup()?;
        Ok::<_, anyhow::Error>(())
    };

    log_error(func()).is_ok()
}

/// Cleans up the environment when the SDK exits, and is recommended to be
/// called when the application exits.
#[no_mangle]
extern "C" fn hylarana_shutdown() {
    log::info!("extern api: hylarana quit");

    let _ = log_error(shutdown());
}

#[repr(C)]
#[derive(Debug)]
#[allow(unused)]
enum HylaranaStrategy {
    Direct,
    Relay,
    Multicast,
}

#[repr(C)]
#[derive(Debug)]
struct HylaranaDescriptor {
    strategy: HylaranaStrategy,
    address: *const c_char,
    mtu: usize,
}

impl TryInto<TransportDescriptor> for HylaranaDescriptor {
    type Error = anyhow::Error;

    fn try_into(self) -> Result<TransportDescriptor, Self::Error> {
        println!("==================== {}, {:?}", Strings::from(self.address).to_string()?, self);

        let address: SocketAddr = Strings::from(self.address).to_string()?.parse()?;

        Ok(TransportDescriptor {
            strategy: match self.strategy {
                HylaranaStrategy::Relay => TransportStrategy::Relay(address),
                HylaranaStrategy::Direct => TransportStrategy::Direct(address),
                HylaranaStrategy::Multicast => TransportStrategy::Multicast(address),
            },
            mtu: self.mtu,
        })
    }
}

#[repr(C)]
#[derive(Clone, Copy)]
#[allow(unused)]
enum RawVideoEncoderType {
    X264,
    Qsv,
    Cuda,
    VideoToolBox,
}

impl Into<VideoEncoderType> for RawVideoEncoderType {
    fn into(self) -> VideoEncoderType {
        match self {
            Self::X264 => VideoEncoderType::X264,
            Self::Qsv => VideoEncoderType::Qsv,
            Self::Cuda => VideoEncoderType::Cuda,
            Self::VideoToolBox => VideoEncoderType::VideoToolBox,
        }
    }
}

/// Video Codec Configuretion.
#[repr(C)]
#[derive(Clone, Copy)]
struct RawVideoDescriptor {
    codec: RawVideoEncoderType,
    frame_rate: u8,
    width: u32,
    height: u32,
    bit_rate: u64,
    key_frame_interval: u32,
}

impl TryInto<VideoDescriptor> for RawVideoDescriptor {
    type Error = anyhow::Error;

    fn try_into(self) -> Result<VideoDescriptor, Self::Error> {
        Ok(VideoDescriptor {
            codec: self.codec.into(),
            key_frame_interval: self.key_frame_interval,
            frame_rate: self.frame_rate,
            width: self.width,
            height: self.height,
            bit_rate: self.bit_rate,
        })
    }
}

/// Audio Codec Configuration.
#[repr(C)]
#[derive(Clone, Copy)]
struct RawAudioDescriptor {
    sample_rate: u64,
    bit_rate: u64,
}

impl Into<AudioDescriptor> for RawAudioDescriptor {
    fn into(self) -> AudioDescriptor {
        AudioDescriptor {
            sample_rate: self.sample_rate,
            bit_rate: self.bit_rate,
        }
    }
}

#[repr(C)]
struct RawSenderSourceDescriptor<T> {
    source: *const RawSource,
    options: T,
}

#[repr(C)]
struct RawSenderDescriptor {
    video: *const RawSenderSourceDescriptor<RawVideoDescriptor>,
    audio: *const RawSenderSourceDescriptor<RawAudioDescriptor>,
    transport: HylaranaDescriptor,
}

impl TryInto<HylaranaSenderDescriptor> for RawSenderDescriptor {
    type Error = anyhow::Error;

    // Both video and audio are optional, so the type conversion here is a bit more
    // complicated.
    #[rustfmt::skip]
    fn try_into(self) -> Result<HylaranaSenderDescriptor, Self::Error> {
        let mut descriptor = HylaranaSenderDescriptor {
            transport: self.transport.try_into()?,
            audio: None,
            video: None,
        };

        if !self.video.is_null() {
            let video = unsafe { &*self.video };
            let options: VideoDescriptor = video.options.try_into()?;

            // Check whether the external parameters are configured correctly to 
            // avoid some clowns inserting some inexplicable parameters.
            anyhow::ensure!(options.width % 4 == 0 && options.width <= 4096, "invalid video width");
            anyhow::ensure!(options.height % 4 == 0 && options.height <= 2560, "invalid video height");
            anyhow::ensure!(options.frame_rate <= 60, "invalid video frame rate");

            descriptor.video = Some(HylaranaSenderSourceDescriptor {
                source: unsafe { &*video.source }.try_into()?,
                options,
            });
        }

        if !self.audio.is_null() {
            let audio = unsafe { &*self.audio };
            descriptor.audio = Some(HylaranaSenderSourceDescriptor {
                source: unsafe { &*audio.source }.try_into()?,
                options: audio.options.try_into()?,
        });
        }

        Ok(descriptor)
    }
}

#[repr(C)]
struct RawSender(HylaranaSender<RawAVFrameStream>);

/// Create a sender, specify a bound NIC address, you can pass callback to
/// get the device screen or sound callback, callback can be null, if it is
/// null then it means no callback data is needed.
#[no_mangle]
extern "C" fn hylarana_create_sender(
    id: *mut c_char,
    options: RawSenderDescriptor,
    sink: RawAVFrameStream,
) -> *const RawSender {
    assert!(!id.is_null());

    log::info!("extern api: hylarana create sender");

    let func = || {
        let options: HylaranaSenderDescriptor = options.try_into()?;
        log::info!("hylarana create options={:?}", options);

        let sender = Hylarana::create_sender(options, sink)?;
        write_c_str(sender.get_id(), id);

        Ok(sender)
    };

    log_error(func())
        .map(|sender| Box::into_raw(Box::new(RawSender(sender))))
        .unwrap_or_else(|_: anyhow::Error| null_mut())
}

/// Close sender.
#[no_mangle]
extern "C" fn hylarana_sender_destroy(sender: *const RawSender) {
    assert!(!sender.is_null());

    log::info!("extern api: hylarana close sender");
    drop(unsafe { Box::from_raw(sender as *mut RawSender) })
}

#[repr(C)]
struct RawReceiver(HylaranaReceiver<RawAVFrameStream>);

#[repr(C)]
#[allow(unused)]
enum RawVideoDecoderType {
    H264,
    D3D11,
    Qsv,
    Cuda,
    VideoToolBox,
}

impl Into<VideoDecoderType> for RawVideoDecoderType {
    fn into(self) -> VideoDecoderType {
        match self {
            Self::H264 => VideoDecoderType::H264,
            Self::D3D11 => VideoDecoderType::D3D11,
            Self::Qsv => VideoDecoderType::Qsv,
            Self::Cuda => VideoDecoderType::Cuda,
            Self::VideoToolBox => VideoDecoderType::VideoToolBox,
        }
    }
}

#[repr(C)]
struct RawReceiverescriptor {
    video: RawVideoDecoderType,
    transport: HylaranaDescriptor,
}

/// Create a receiver, specify a bound NIC address, you can pass callback to
/// get the sender's screen or sound callback, callback can not be null.
#[no_mangle]
extern "C" fn hylarana_create_receiver(
    id: *const c_char,
    options: RawReceiverescriptor,
    sink: RawAVFrameStream,
) -> *const RawReceiver {
    assert!(!id.is_null());

    log::info!("extern api: hylarana create receiver");

    let func = || {
        Ok::<_, anyhow::Error>(Hylarana::create_receiver(
            Strings::from(id).to_string()?,
            HylaranaReceiverDescriptor {
                transport: options.transport.try_into()?,
                video: options.video.into(),
            },
            sink,
        )?)
    };

    log_error(func())
        .map(|receiver| Box::into_raw(Box::new(RawReceiver(receiver))))
        .unwrap_or_else(|_| null_mut())
}

/// Close receiver.
#[no_mangle]
extern "C" fn hylarana_receiver_destroy(receiver: *const RawReceiver) {
    assert!(!receiver.is_null());

    log::info!("extern api: hylarana close receiver");
    drop(unsafe { Box::from_raw(receiver as *mut RawReceiver) })
}

#[repr(C)]
#[allow(unused)]
enum RawGraphicsBackend {
    Direct3D11,
    WebGPU,
}

impl Into<GraphicsBackend> for RawGraphicsBackend {
    fn into(self) -> GraphicsBackend {
        match self {
            Self::Direct3D11 => GraphicsBackend::Direct3D11,
            Self::WebGPU => GraphicsBackend::WebGPU,
        }
    }
}
