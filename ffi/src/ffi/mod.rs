mod capture;
mod discovery;
mod observer;
mod player;

use std::{ffi::c_char, fmt::Debug, net::SocketAddr, ptr::null_mut};

use self::{
    capture::RawSource,
    observer::RawAVFrameStream,
    player::{Player, RawPlayerOptions},
};

use hylarana::{
    shutdown, startup, AudioOptions, Hylarana, HylaranaReceiver, HylaranaReceiverCodecOptions,
    HylaranaReceiverOptions, HylaranaSender, HylaranaSenderMediaOptions, HylaranaSenderOptions,
    HylaranaSenderTrackOptions, TransportOptions, TransportStrategy, VideoDecoderType,
    VideoEncoderType, VideoOptions,
};

use hylarana_common::{logger, strings::PSTR};

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
    log_error((|| {
        logger::init_logger(log::LevelFilter::Info, None)?;

        startup()?;
        Ok::<_, anyhow::Error>(())
    })())
    .is_ok()
}

/// Cleans up the environment when the SDK exits, and is recommended to be
/// called when the application exits.
#[no_mangle]
extern "C" fn hylarana_shutdown() {
    log::info!("extern api: hylarana quit");

    let _ = log_error(shutdown());
}

#[repr(C)]
#[allow(unused)]
enum RawTransportStrategy {
    Direct,
    Relay,
    Multicast,
}

#[repr(C)]
struct RawTransportOptions {
    strategy: RawTransportStrategy,
    address: *const c_char,
    mtu: usize,
}

impl TryInto<TransportOptions> for RawTransportOptions {
    type Error = anyhow::Error;

    fn try_into(self) -> Result<TransportOptions, Self::Error> {
        let address: SocketAddr = PSTR::from(self.address).to_string()?.parse()?;

        Ok(TransportOptions {
            strategy: match self.strategy {
                RawTransportStrategy::Relay => TransportStrategy::Relay(address),
                RawTransportStrategy::Direct => TransportStrategy::Direct(address),
                RawTransportStrategy::Multicast => TransportStrategy::Multicast(address),
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
    VideoToolBox,
}

impl Into<VideoEncoderType> for RawVideoEncoderType {
    fn into(self) -> VideoEncoderType {
        match self {
            Self::X264 => VideoEncoderType::X264,
            Self::Qsv => VideoEncoderType::Qsv,
            Self::VideoToolBox => VideoEncoderType::VideoToolBox,
        }
    }
}

/// Video Codec Configuretion.
#[repr(C)]
#[derive(Clone, Copy)]
struct RawVideoOptions {
    codec: RawVideoEncoderType,
    frame_rate: u8,
    width: u32,
    height: u32,
    bit_rate: u64,
    key_frame_interval: u32,
}

impl TryInto<VideoOptions> for RawVideoOptions {
    type Error = anyhow::Error;

    fn try_into(self) -> Result<VideoOptions, Self::Error> {
        Ok(VideoOptions {
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
struct RawAudioOptions {
    sample_rate: u64,
    bit_rate: u64,
}

impl Into<AudioOptions> for RawAudioOptions {
    fn into(self) -> AudioOptions {
        AudioOptions {
            sample_rate: self.sample_rate,
            bit_rate: self.bit_rate,
        }
    }
}

#[repr(C)]
struct RawSenderTrackOptions<T> {
    source: *const RawSource,
    options: T,
}

#[repr(C)]
struct RawSenderMediaOptions {
    video: *const RawSenderTrackOptions<RawVideoOptions>,
    audio: *const RawSenderTrackOptions<RawAudioOptions>,
}

impl TryInto<HylaranaSenderMediaOptions> for RawSenderMediaOptions {
    type Error = anyhow::Error;

    fn try_into(self) -> Result<HylaranaSenderMediaOptions, Self::Error> {
        Ok(HylaranaSenderMediaOptions {
            video: if !self.video.is_null() {
                let video = unsafe { &*self.video };
                Some(HylaranaSenderTrackOptions {
                    source: unsafe { &*video.source }.try_into()?,
                    options: video.options.try_into()?,
                })
            } else {
                None
            },
            audio: if !self.audio.is_null() {
                let audio = unsafe { &*self.audio };
                Some(HylaranaSenderTrackOptions {
                    source: unsafe { &*audio.source }.try_into()?,
                    options: audio.options.try_into()?,
                })
            } else {
                None
            },
        })
    }
}

#[repr(C)]
struct RawSenderOptions {
    media: RawSenderMediaOptions,
    transport: RawTransportOptions,
}

impl TryInto<HylaranaSenderOptions> for RawSenderOptions {
    type Error = anyhow::Error;

    // Both video and audio are optional, so the type conversion here is a bit more
    // complicated.
    #[rustfmt::skip]
    fn try_into(self) -> Result<HylaranaSenderOptions, Self::Error> {
        Ok(HylaranaSenderOptions {
            transport: self.transport.try_into()?,
            media: self.media.try_into()?,
        })
    }
}

#[repr(C)]
struct RawSender(HylaranaSender<RawAVFrameStream>);

/// Create a sender, specify a bound NIC address, you can pass callback to
/// get the device screen or sound callback, callback can be null, if it is
/// null then it means no callback data is needed.
#[no_mangle]
extern "C" fn hylarana_create_sender(
    options: RawSenderOptions,
    sink: RawAVFrameStream,
    id: *mut c_char,
) -> *const RawSender {
    assert!(!id.is_null());

    log::info!("extern api: hylarana create sender");

    log_error((|| {
        let options: HylaranaSenderOptions = options.try_into()?;
        log::info!("create sender options={:?}", options);

        let sender = Hylarana::create_sender(options, sink)?;
        PSTR::strcpy(sender.get_id(), id);

        Ok(sender)
    })())
    .map(|it| Box::into_raw(Box::new(RawSender(it))))
    .unwrap_or_else(|_: anyhow::Error| null_mut())
}

/// Destroy sender.
#[no_mangle]
extern "C" fn hylarana_sender_destroy(sender: *mut RawSender) {
    assert!(!sender.is_null());

    log::info!("extern api: hylarana close sender");

    drop(unsafe { Box::from_raw(sender) })
}

#[repr(C)]
struct RawSenderWithPlayer(HylaranaSender<Player>);

/// Create the sender. the difference is that this function creates the player
/// together, you don't need to implement the stream sink manually, the player
/// manages it automatically.
#[no_mangle]
extern "C" fn hylarana_create_sender_with_player(
    options: RawSenderOptions,
    player_options: RawPlayerOptions,
    id: *mut c_char,
) -> *const RawSenderWithPlayer {
    assert!(!id.is_null());

    log::info!("extern api: hylarana create sender with player");

    log_error((|| {
        let options: HylaranaSenderOptions = options.try_into()?;
        log::info!("create sender options={:?}", options);

        let sender = Hylarana::create_sender(options, player_options.create_player()?)?;
        PSTR::strcpy(sender.get_id(), id);

        Ok(sender)
    })())
    .map(|it| Box::into_raw(Box::new(RawSenderWithPlayer(it))))
    .unwrap_or_else(|_: anyhow::Error| null_mut())
}

/// Destroy sender with player.
#[no_mangle]
extern "C" fn hylarana_sender_with_player_destroy(sender: *mut RawSenderWithPlayer) {
    assert!(!sender.is_null());

    log::info!("extern api: hylarana close sender with player");

    drop(unsafe { Box::from_raw(sender) })
}

#[repr(C)]
#[allow(unused)]
enum RawVideoDecoderType {
    H264,
    D3D11,
    Qsv,
    VideoToolBox,
}

impl Into<VideoDecoderType> for RawVideoDecoderType {
    fn into(self) -> VideoDecoderType {
        match self {
            Self::H264 => VideoDecoderType::H264,
            Self::D3D11 => VideoDecoderType::D3D11,
            Self::Qsv => VideoDecoderType::Qsv,
            Self::VideoToolBox => VideoDecoderType::VideoToolBox,
        }
    }
}

#[repr(C)]
struct RawReceiverCodecOptions {
    video: RawVideoDecoderType,
}

#[repr(C)]
struct RawReceiverOptions {
    codec: RawReceiverCodecOptions,
    transport: RawTransportOptions,
}

#[repr(C)]
struct RawReceiver(HylaranaReceiver<RawAVFrameStream>);

/// Create a receiver, specify a bound NIC address, you can pass callback to
/// get the sender's screen or sound callback, callback can not be null.
#[no_mangle]
extern "C" fn hylarana_create_receiver(
    id: *const c_char,
    options: RawReceiverOptions,
    sink: RawAVFrameStream,
) -> *const RawReceiver {
    assert!(!id.is_null());

    log::info!("extern api: hylarana create receiver");

    log_error((|| {
        Ok::<_, anyhow::Error>(Hylarana::create_receiver(
            PSTR::from(id).to_string()?,
            HylaranaReceiverOptions {
                transport: options.transport.try_into()?,
                codec: HylaranaReceiverCodecOptions {
                    video: options.codec.video.into(),
                },
            },
            sink,
        )?)
    })())
    .map(|it| Box::into_raw(Box::new(RawReceiver(it))))
    .unwrap_or_else(|_| null_mut())
}

/// Destroy receiver.
#[no_mangle]
extern "C" fn hylarana_receiver_destroy(receiver: *mut RawReceiver) {
    assert!(!receiver.is_null());

    log::info!("extern api: hylarana close receiver");

    drop(unsafe { Box::from_raw(receiver) })
}

#[repr(C)]
struct RawReceiverWithPlayer(HylaranaReceiver<Player>);

/// Create the receiver. the difference is that this function creates the player
/// together, you don't need to implement the stream sink manually, the player
/// manages it automatically.
#[no_mangle]
extern "C" fn hylarana_create_receiver_with_player(
    id: *const c_char,
    options: RawReceiverOptions,
    player_options: RawPlayerOptions,
) -> *const RawReceiverWithPlayer {
    assert!(!id.is_null());

    log::info!("extern api: hylarana create receiver with player");

    log_error((|| {
        Ok::<_, anyhow::Error>(Hylarana::create_receiver(
            PSTR::from(id).to_string()?,
            HylaranaReceiverOptions {
                transport: options.transport.try_into()?,
                codec: HylaranaReceiverCodecOptions {
                    video: options.codec.video.into(),
                },
            },
            player_options.create_player()?,
        )?)
    })())
    .map(|it| Box::into_raw(Box::new(RawReceiverWithPlayer(it))))
    .unwrap_or_else(|_| null_mut())
}

/// Destroy receiver with player.
#[no_mangle]
extern "C" fn hylarana_receiver_with_player_destroy(receiver: *mut RawReceiverWithPlayer) {
    assert!(!receiver.is_null());

    log::info!("extern api: hylarana close receiver with player");

    drop(unsafe { Box::from_raw(receiver) })
}
