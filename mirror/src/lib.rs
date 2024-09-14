mod audio;
mod receiver;
mod sender;
mod video;

pub use self::{
    receiver::{Receiver, ReceiverDescriptor},
    video::{win32::VideoRenderDescriptor, Size},
};

#[cfg(not(target_os = "macos"))]
pub use self::sender::{AudioDescriptor, Sender, SenderDescriptor, VideoDescriptor};

#[cfg(target_os = "windows")]
use self::video::win32::VideoRender;

#[cfg(not(target_os = "macos"))]
use std::sync::RwLock;

use anyhow::Result;
use audio::AudioPlayer;
use frame::{AudioFrame, VideoFrame};
use transport::{Transport, TransportDescriptor};
use utils::logger;

#[cfg(target_os = "windows")]
use utils::win32::{
    set_process_priority, shutdown as win32_shutdown, startup as win32_startup, Direct3DDevice,
    ProcessPriority,
};

#[cfg(target_os = "windows")]
pub use windows::Win32::Foundation::HWND;

#[cfg(target_os = "windows")]
pub(crate) static DIRECT_3D_DEVICE: RwLock<Option<Direct3DDevice>> = RwLock::new(None);

/// Initialize the environment, which must be initialized before using the SDK.
#[rustfmt::skip]
pub fn startup() -> Result<()> {
    logger::init(
        log::LevelFilter::Info,
        if cfg!(debug_assertions) {
            Some("mirror.log")
        } else {
            None
        },
    )?;

    log::info!("mirror startup");

    #[cfg(target_os = "windows")]
    {
        win32_startup()?;
    }

    std::panic::set_hook(Box::new(|info| {
        log::error!("{:?}", info);
    }));

    // In order to prevent other programs from affecting the delay performance of
    // the current program, set the priority of the current process to high.
    #[cfg(target_os = "windows")]
    {
        if set_process_priority(ProcessPriority::High).is_err() {
            log::error!(
                "failed to set current process priority, Maybe it's \
                because you didn't run it with administrator privileges."
            );
        }
    }

    codec::startup();
    log::info!("codec initialized");

    transport::startup();
    log::info!("transport initialized");

    log::info!("all initialized");
    Ok(())
}

/// Cleans up the environment when the SDK exits, and is recommended to be
/// called when the application exits.
pub fn shutdown() -> Result<()> {
    log::info!("mirror shutdown");

    #[cfg(target_os = "windows")]
    win32_shutdown()?;

    codec::shutdown();
    transport::shutdown();

    Ok(())
}

pub struct FrameSink {
    /// Callback occurs when the video frame is updated. The video frame format
    /// is fixed to NV12. Be careful not to call blocking methods inside the
    /// callback, which will seriously slow down the encoding and decoding
    /// pipeline.
    ///
    /// YCbCr (NV12)
    ///
    /// YCbCr, Y′CbCr, or Y Pb/Cb Pr/Cr, also written as YCBCR or Y′CBCR, is a
    /// family of color spaces used as a part of the color image pipeline in
    /// video and digital photography systems. Y′ is the luma component and
    /// CB and CR are the blue-difference and red-difference chroma
    /// components. Y′ (with prime) is distinguished from Y, which is
    /// luminance, meaning that light intensity is nonlinearly encoded based
    /// on gamma corrected RGB primaries.
    ///
    /// Y′CbCr color spaces are defined by a mathematical coordinate
    /// transformation from an associated RGB primaries and white point. If
    /// the underlying RGB color space is absolute, the Y′CbCr color space
    /// is an absolute color space as well; conversely, if the RGB space is
    /// ill-defined, so is Y′CbCr. The transformation is defined in
    /// equations 32, 33 in ITU-T H.273. Nevertheless that rule does not
    /// apply to P3-D65 primaries used by Netflix with BT.2020-NCL matrix,
    /// so that means matrix was not derived from primaries, but now Netflix
    /// allows BT.2020 primaries (since 2021). The same happens with
    /// JPEG: it has BT.601 matrix derived from System M primaries, yet the
    /// primaries of most images are BT.709.
    pub video: Box<dyn Fn(&VideoFrame) -> bool + Send + Sync>,
    /// Callback is called when the audio frame is updated. The audio frame
    /// format is fixed to PCM. Be careful not to call blocking methods inside
    /// the callback, which will seriously slow down the encoding and decoding
    /// pipeline.
    ///
    /// Pulse-code modulation
    ///
    /// Pulse-code modulation (PCM) is a method used to digitally represent
    /// analog signals. It is the standard form of digital audio in
    /// computers, compact discs, digital telephony and other digital audio
    /// applications. In a PCM stream, the amplitude of the analog signal is
    /// sampled at uniform intervals, and each sample is quantized to the
    /// nearest value within a range of digital steps.
    ///
    /// Linear pulse-code modulation (LPCM) is a specific type of PCM in which
    /// the quantization levels are linearly uniform. This is in contrast to
    /// PCM encodings in which quantization levels vary as a function of
    /// amplitude (as with the A-law algorithm or the μ-law algorithm).
    /// Though PCM is a more general term, it is often used to describe data
    /// encoded as LPCM.
    ///
    /// A PCM stream has two basic properties that determine the stream's
    /// fidelity to the original analog signal: the sampling rate, which is
    /// the number of times per second that samples are taken; and the bit
    /// depth, which determines the number of possible digital values that
    /// can be used to represent each sample.
    pub audio: Box<dyn Fn(&AudioFrame) -> bool + Send + Sync>,
    /// Callback when the sender is closed. This may be because the external
    /// side actively calls the close, or the audio and video packets cannot be
    /// sent (the network is disconnected), etc.
    pub close: Box<dyn Fn() + Send + Sync>,
}

pub struct Mirror(Transport);

impl Mirror {
    pub fn new(options: TransportDescriptor) -> Result<Self> {
        log::info!("create mirror: options={:?}", options);

        // Check if the D3D device has been created. If not, create a global one.
        #[cfg(target_os = "windows")]
        {
            if DIRECT_3D_DEVICE.read().unwrap().is_none() {
                DIRECT_3D_DEVICE
                    .write()
                    .unwrap()
                    .replace(Direct3DDevice::new()?);
            }
        }

        Ok(Self(Transport::new(options)?))
    }

    /// get direct3d device
    #[cfg(target_os = "windows")]
    pub fn get_direct3d_device(&self) -> Option<Direct3DDevice> {
        DIRECT_3D_DEVICE.read().unwrap().clone()
    }

    /// Create a sender, specify a bound NIC address, you can pass callback to
    /// get the device screen or sound callback, callback can be null, if it is
    /// null then it means no callback data is needed.
    #[cfg(not(target_os = "macos"))]
    pub fn create_sender(
        &self,
        id: u32,
        options: SenderDescriptor,
        sink: FrameSink,
    ) -> Result<Sender> {
        log::info!("create sender: id={}, options={:?}", id, options);

        let sender = Sender::new(options, sink)?;
        self.0.create_sender(id, &sender.adapter)?;
        Ok(sender)
    }

    /// Create a receiver, specify a bound NIC address, you can pass callback to
    /// get the sender's screen or sound callback, callback can not be null.
    pub fn create_receiver(
        &self,
        id: u32,
        options: ReceiverDescriptor,
        sink: FrameSink,
    ) -> Result<Receiver> {
        log::info!("create receiver: id={}, options={:?}", id, options);

        let receiver = Receiver::new(options, sink)?;
        self.0.create_receiver(id, &receiver.adapter)?;
        Ok(receiver)
    }
}

pub struct RenderDescriptor {
    pub size: Size,
    #[cfg(target_os = "windows")]
    pub window_handle: HWND,
}

pub struct Render {
    audio: AudioPlayer,
    #[cfg(target_os = "windows")]
    video: VideoRender,
}

impl Render {
    pub fn new(options: RenderDescriptor) -> Result<Self> {
        Ok(Self {
            audio: AudioPlayer::new()?,
            #[cfg(target_os = "windows")]
            video: VideoRender::new(VideoRenderDescriptor {
                size: options.size.into(),
                window_handle: options.window_handle,
                direct3d: DIRECT_3D_DEVICE
                    .read()
                    .unwrap()
                    .clone()
                    .expect("D3D device was not initialized successfully!"),
            })?,
        })
    }

    pub fn on_video(&mut self, frame: &VideoFrame) -> Result<()> {
        self.video.send(frame)
    }

    pub fn on_audio(&mut self, frame: &AudioFrame) -> Result<()> {
        self.audio.send(frame)
    }
}