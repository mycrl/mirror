mod receiver;
mod render;

#[cfg(any(target_os = "windows", target_os = "linux"))]
mod sender;

pub use self::{
    receiver::{MirrorReceiver, MirrorReceiverDescriptor, ReceiverError},
    render::{Backend as VideoRenderBackend, RendererError},
    sender::{AudioDescriptor, MirrorSender, MirrorSenderDescriptor, SenderError, VideoDescriptor},
};

use self::render::{AudioRender, VideoRender};

use graphics::SurfaceTarget;
use parking_lot::Mutex;
use thiserror::Error;
use transport::Transport;

#[cfg(target_os = "windows")]
use parking_lot::RwLock;

pub use capture::{Capture, Source, SourceType};
pub use codec::{VideoDecoderType, VideoEncoderType};
pub use common::{
    frame::{AudioFrame, VideoFormat, VideoFrame},
    Size,
};

pub use graphics::raw_window_handle;
pub use transport::TransportDescriptor;

#[cfg(target_os = "windows")]
use common::win32::{
    set_process_priority, shutdown as win32_shutdown, startup as win32_startup, Direct3DDevice,
    ProcessPriority,
};

#[cfg(target_os = "windows")]
pub(crate) static DIRECT_3D_DEVICE: RwLock<Option<Direct3DDevice>> = RwLock::new(None);

#[derive(Debug, Error)]
pub enum MirrorError {
    #[error(transparent)]
    #[cfg(target_os = "windows")]
    Win32Error(#[from] common::win32::windows::core::Error),
    #[error(transparent)]
    TransportError(#[from] std::io::Error),
}

/// Initialize the environment, which must be initialized before using the SDK.
pub fn startup() -> Result<(), MirrorError> {
    log::info!("mirror startup");

    #[cfg(target_os = "windows")]
    if let Err(e) = win32_startup() {
        log::warn!("{:?}", e);
    }

    // In order to prevent other programs from affecting the delay performance of
    // the current program, set the priority of the current process to high.
    #[cfg(target_os = "windows")]
    if set_process_priority(ProcessPriority::High).is_err() {
        log::error!(
            "failed to set current process priority, Maybe it's \
            because you didn't run it with administrator privileges."
        );
    }

    #[cfg(target_os = "linux")]
    capture::startup();

    codec::startup();
    log::info!("codec initialized");

    transport::startup();
    log::info!("transport initialized");

    log::info!("all initialized");
    Ok(())
}

/// Cleans up the environment when the SDK exits, and is recommended to be
/// called when the application exits.
pub fn shutdown() -> Result<(), MirrorError> {
    log::info!("mirror shutdown");

    codec::shutdown();
    transport::shutdown();

    #[cfg(target_os = "windows")]
    if let Err(e) = win32_shutdown() {
        log::warn!("{:?}", e);
    }

    Ok(())
}

pub trait Close: Sync + Send {
    /// Callback when the sender is closed. This may be because the external
    /// side actively calls the close, or the audio and video packets cannot be
    /// sent (the network is disconnected), etc.
    fn close(&self);
}

pub trait AVFrameSink: Sync + Send {
    /// Callback occurs when the video frame is updated. The video frame format
    /// is fixed to NV12. Be careful not to call blocking methods inside the
    /// callback, which will seriously slow down the encoding and decoding
    /// pipeline.
    #[allow(unused_variables)]
    fn video(&self, frame: &VideoFrame) -> bool {
        true
    }

    /// Callback is called when the audio frame is updated. The audio frame
    /// format is fixed to PCM. Be careful not to call blocking methods inside
    /// the callback, which will seriously slow down the encoding and decoding
    /// pipeline.
    #[allow(unused_variables)]
    fn audio(&self, frame: &AudioFrame) -> bool {
        true
    }
}

pub trait AVFrameStream: AVFrameSink + Close {}

pub struct Mirror(Transport);

impl Mirror {
    pub fn new(options: TransportDescriptor) -> Result<Self, MirrorError> {
        log::info!("create mirror: options={:?}", options);

        // Check if the D3D device has been created. If not, create a global one.
        #[cfg(target_os = "windows")]
        {
            if DIRECT_3D_DEVICE.read().is_none() {
                DIRECT_3D_DEVICE.write().replace(Direct3DDevice::new()?);
            }
        }

        Ok(Self(Transport::new(options)?))
    }

    /// Create a sender, specify a bound NIC address, you can pass callback to
    /// get the device screen or sound callback, callback can be null, if it is
    /// null then it means no callback data is needed.
    #[cfg(any(target_os = "windows", target_os = "linux"))]
    pub fn create_sender<T: AVFrameStream + 'static>(
        &self,
        id: u32,
        options: MirrorSenderDescriptor,
        sink: T,
    ) -> Result<MirrorSender, SenderError> {
        log::info!("create sender: id={}, options={:?}", id, options);

        let sender = MirrorSender::new(options, sink)?;
        self.0.create_sender(id, &sender.adapter)?;
        Ok(sender)
    }

    /// Create a receiver, specify a bound NIC address, you can pass callback to
    /// get the sender's screen or sound callback, callback can not be null.
    pub fn create_receiver<T: AVFrameStream + 'static>(
        &self,
        id: u32,
        options: MirrorReceiverDescriptor,
        sink: T,
    ) -> Result<MirrorReceiver, ReceiverError> {
        log::info!("create receiver: id={}, options={:?}", id, options);

        let receiver = MirrorReceiver::new(options, sink)?;
        self.0.create_receiver(id, &receiver.adapter)?;
        Ok(receiver)
    }
}

/// Renderer for video frames and audio frames.
///
/// Typically, the player underpinnings for audio and video are implementedin
/// hardware, but not always, the underpinnings automatically select the adapter
/// and fall back to the software adapter if the hardware adapter is
/// unavailable, for video this can be done by enabling the dx11 feature tobe
/// implemented with Direct3D 11 Graphics, which works fine on some very old
/// devices.
pub struct Render<'a>(Mutex<VideoRender<'a>>, Mutex<AudioRender>);

impl<'a> Render<'a> {
    pub fn new<T: Into<SurfaceTarget<'a>>>(
        backend: VideoRenderBackend,
        window: T,
        size: Size,
    ) -> Result<Self, RendererError> {
        Ok(Self(
            Mutex::new(VideoRender::new(backend, window, size)?),
            Mutex::new(AudioRender::new()?),
        ))
    }
}

impl<'a> AVFrameSink for Render<'a> {
    /// Renders the audio frame, note that a queue is maintained internally,
    /// here it just pushes the audio to the playback queue, and if the queue is
    /// empty, it fills the mute data to the player by default, so you need to
    /// pay attention to the push rate.
    fn audio(&self, frame: &AudioFrame) -> bool {
        if let Err(e) = self.1.lock().send(frame) {
            log::error!("{:?}", e);

            return false;
        }

        true
    }

    /// Renders video frames and can automatically handle rendering of hardware
    /// textures and rendering textures.
    fn video(&self, frame: &VideoFrame) -> bool {
        if let Err(e) = self.0.lock().send(frame) {
            log::error!("{:?}", e);

            return false;
        }

        true
    }
}
