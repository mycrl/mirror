mod receiver;
mod sender;

pub use self::{
    receiver::{Receiver, ReceiverDescriptor, ReceiverError},
    sender::{AudioDescriptor, Sender, SenderDescriptor, SenderError, VideoDescriptor},
};

use std::{
    slice::from_raw_parts,
    sync::{
        mpsc::{channel, Receiver as MpscReceiver, Sender as MpscSender},
        Arc,
    },
};

pub use capture::{Capture, Source, SourceType};
pub use codec::{VideoDecoderType, VideoEncoderType};
pub use common::{
    frame::{AudioFrame, VideoFormat, VideoFrame, VideoSubFormat},
    Size,
};

pub use graphics::raw_window_handle;
pub use transport::TransportDescriptor;

use cpal::{
    traits::{DeviceTrait, HostTrait, StreamTrait},
    Stream, StreamConfig, StreamError,
};

#[cfg(target_os = "windows")]
use common::win32::{
    d3d_texture_borrowed_raw, set_process_priority, shutdown as win32_shutdown,
    startup as win32_startup, windows::Win32::Foundation::HWND, Direct3DDevice, ProcessPriority,
};

#[cfg(target_os = "windows")]
use graphics::dx11::Dx11Renderer;
use graphics::{
    Renderer as WgpuRenderer, RendererOptions as WgpuRendererOptions, SurfaceTarget, Texture,
    Texture2DBuffer, Texture2DResource,
};

use parking_lot::{Mutex, RwLock};
use resample::AudioResampler;
use thiserror::Error;
use transport::Transport;

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
    pub fn create_sender<T: AVFrameStream + 'static>(
        &self,
        id: u32,
        options: SenderDescriptor,
        sink: T,
    ) -> Result<Sender, SenderError> {
        log::info!("create sender: id={}, options={:?}", id, options);

        let sender = Sender::new(options, sink)?;
        self.0.create_sender(id, &sender.adapter)?;
        Ok(sender)
    }

    /// Create a receiver, specify a bound NIC address, you can pass callback to
    /// get the sender's screen or sound callback, callback can not be null.
    pub fn create_receiver<T: AVFrameStream + 'static>(
        &self,
        id: u32,
        options: ReceiverDescriptor,
        sink: T,
    ) -> Result<Receiver, ReceiverError> {
        log::info!("create receiver: id={}, options={:?}", id, options);

        let receiver = Receiver::new(options, sink)?;
        self.0.create_receiver(id, &receiver.adapter)?;
        Ok(receiver)
    }
}

#[derive(Debug, Error)]
pub enum RendererError {
    #[error("no output device available")]
    AudioNotFoundOutputDevice,
    #[error(transparent)]
    AudioDefaultStreamConfigError(#[from] cpal::DefaultStreamConfigError),
    #[error(transparent)]
    AudioBuildStreamError(#[from] cpal::BuildStreamError),
    #[error(transparent)]
    AudioResamplerConstructionError(#[from] resample::ResamplerConstructionError),
    #[error(transparent)]
    AudioStreamError(#[from] cpal::StreamError),
    #[error(transparent)]
    AudioPlayStreamError(#[from] cpal::PlayStreamError),
    #[error(transparent)]
    AudioResampleError(#[from] resample::ResampleError),
    #[error("send audio queue error")]
    AudioSendQueueError,
    #[error(transparent)]
    #[cfg(target_os = "windows")]
    VideoDx11GraphicsError(#[from] graphics::dx11::Dx11GraphicsError),
    #[error(transparent)]
    VideoGraphicsError(#[from] graphics::GraphicsError),
    #[error("invalid d3d11texture2d texture")]
    #[cfg(target_os = "windows")]
    VideoInvalidD3D11Texture,
}

pub struct AudioRender {
    stream: Stream,
    config: StreamConfig,
    queue: MpscSender<Vec<i16>>,
    sampler: Option<AudioResampler>,
    current_error: Arc<RwLock<Option<StreamError>>>,
}

unsafe impl Send for AudioRender {}
unsafe impl Sync for AudioRender {}

impl AudioRender {
    pub fn new() -> Result<Self, RendererError> {
        let host = cpal::default_host();
        let device = host
            .default_output_device()
            .ok_or_else(|| RendererError::AudioNotFoundOutputDevice)?;
        let config: StreamConfig = device.default_output_config()?.into();
        let current_error: Arc<RwLock<Option<StreamError>>> = Default::default();

        let (queue, rx) = channel();
        let stream = {
            let current_error_ = Arc::downgrade(&current_error);
            let mut queue = AudioQueue {
                queue: rx,
                current_chunk: None,
            };

            device.build_output_stream(
                &config,
                move |data: &mut [i16], _: &cpal::OutputCallbackInfo| {
                    queue.read(data, config.channels as usize);
                },
                move |err| {
                    if let Some(current_error) = current_error_.upgrade() {
                        current_error.write().replace(err);
                    }
                },
                None,
            )?
        };

        Ok(Self {
            stream,
            queue,
            config,
            current_error,
            sampler: None,
        })
    }

    /// Push an audio clip to the queue.
    pub fn send(&mut self, frame: &AudioFrame) -> Result<(), RendererError> {
        if self.current_error.read().is_some() {
            if let Some(e) = self.current_error.write().take() {
                return Err(RendererError::AudioStreamError(e));
            }
        }

        if self.sampler.is_none() {
            self.sampler = Some(AudioResampler::new(
                frame.sample_rate as f64,
                self.config.sample_rate.0 as f64,
                frame.frames as usize,
            )?);

            // Start playing audio by first push.
            self.stream.play()?;
        }

        if let Some(sampler) = &mut self.sampler {
            self.queue
                .send(
                    sampler
                        .resample(
                            unsafe { from_raw_parts(frame.data, frame.frames as usize) },
                            1,
                        )?
                        .to_vec(),
                )
                .map_err(|_| RendererError::AudioSendQueueError)?;
        }

        Ok(())
    }
}

impl Drop for AudioRender {
    fn drop(&mut self) {
        let _ = self.stream.pause();
    }
}

struct AudioQueue {
    queue: MpscReceiver<Vec<i16>>,
    current_chunk: Option<std::vec::IntoIter<i16>>,
}

static MUTE_BUF: [i16; 48000] = [0; 48000];

impl AudioQueue {
    fn read(&mut self, output: &mut [i16], channels: usize) {
        let mut index = 0;

        // Copy from queue to player
        'a: while index < output.len() {
            // Check if the buffer is empty
            if let Some(chunk) = &mut self.current_chunk {
                loop {
                    // Writing to the player buffer is complete
                    if index >= output.len() {
                        break;
                    }

                    // Read data from the queue buffer and write it to the player buffer. If the
                    // queue buffer is empty, jump to the step of updating the buffer.
                    if let Some(item) = chunk.next() {
                        for i in 0..channels {
                            output[index + i] = item;
                        }

                        index += channels;
                    } else {
                        self.current_chunk = None;
                        continue 'a;
                    }
                }
            } else {
                // If the buffer is empty, take another one from the queue and put it into the
                // buffer. If the queue is empty, fill it directly with silent data.
                if let Ok(chunk) = self.queue.try_recv() {
                    self.current_chunk = Some(chunk.into_iter());
                } else {
                    output.copy_from_slice(&MUTE_BUF[..output.len()]);
                    break;
                }
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GraphicsBackend {
    Direct3D11,
    WebGPU,
}

pub enum VideoRender<'a> {
    WebGPU(WgpuRenderer<'a>),
    #[cfg(target_os = "windows")]
    Direct3D11(Dx11Renderer),
}

impl<'a> VideoRender<'a> {
    pub fn new<T: Into<SurfaceTarget<'a>>>(
        backend: GraphicsBackend,
        window: T,
        size: Size,
    ) -> Result<Self, RendererError> {
        log::info!(
            "create video render, backend={:?}, size={:?}",
            backend,
            size
        );

        #[cfg(target_os = "windows")]
        let direct3d = crate::DIRECT_3D_DEVICE.read().as_ref().unwrap().clone();

        Ok(match backend {
            #[cfg(target_os = "windows")]
            GraphicsBackend::Direct3D11 => Self::Direct3D11(Dx11Renderer::new(
                match window.into() {
                    SurfaceTarget::Window(window) => match window.window_handle().unwrap().as_raw()
                    {
                        graphics::raw_window_handle::RawWindowHandle::Win32(window) => {
                            HWND(window.hwnd.get() as _)
                        }
                        _ => unimplemented!(
                            "what happened? why is the dx11 renderer enabled on linux?"
                        ),
                    },
                    _ => {
                        unimplemented!("the renderer does not support non-windowed render targets")
                    }
                },
                size,
                direct3d,
            )?),
            GraphicsBackend::WebGPU => Self::WebGPU(WgpuRenderer::new(WgpuRendererOptions {
                #[cfg(target_os = "windows")]
                direct3d,
                window,
                size,
            })?),
            _ => unimplemented!("not supports the {:?} backend", backend),
        })
    }

    pub fn send(&mut self, frame: &VideoFrame) -> Result<(), RendererError> {
        match frame.sub_format {
            VideoSubFormat::D3D11 => {
                #[cfg(target_os = "windows")]
                {
                    let dx_tex = d3d_texture_borrowed_raw(&(frame.data[0] as *mut _))
                        .cloned()
                        .ok_or_else(|| RendererError::VideoInvalidD3D11Texture)?;

                    let texture = Texture2DResource::Texture(graphics::Texture2DRaw::Direct3D11(
                        &dx_tex,
                        frame.data[1] as u32,
                    ));

                    let texture = match frame.format {
                        VideoFormat::BGRA => Texture::Bgra(texture),
                        VideoFormat::RGBA => Texture::Rgba(texture),
                        VideoFormat::NV12 => Texture::Nv12(texture),
                        VideoFormat::I420 => unimplemented!("no hardware texture for I420"),
                    };

                    match self {
                        Self::Direct3D11(render) => render.submit(texture)?,
                        Self::WebGPU(render) => render.submit(texture)?,
                    }
                }
            }
            VideoSubFormat::SW => {
                let buffers = match frame.format {
                    // RGBA stands for red green blue alpha. While it is sometimes described as a
                    // color space, it is actually a three-channel RGB color model supplemented
                    // with a fourth alpha channel. Alpha indicates how opaque each pixel is and
                    // allows an image to be combined over others using alpha compositing, with
                    // transparent areas and anti-aliasing of the edges of opaque regions. Each
                    // pixel is a 4D vector.
                    //
                    // The term does not define what RGB color space is being used. It also does
                    // not state whether or not the colors are premultiplied by the alpha value,
                    // and if they are it does not state what color space that premultiplication
                    // was done in. This means more information than just "RGBA" is needed to
                    // determine how to handle an image.
                    //
                    // In some contexts the abbreviation "RGBA" means a specific memory layout
                    // (called RGBA8888 below), with other terms such as "BGRA" used for
                    // alternatives. In other contexts "RGBA" means any layout.
                    VideoFormat::BGRA | VideoFormat::RGBA => [
                        unsafe {
                            std::slice::from_raw_parts(
                                frame.data[0] as *const _,
                                frame.linesize[0] * frame.height as usize,
                            )
                        },
                        &[],
                        &[],
                    ],
                    // YCbCr, Y′CbCr, or Y Pb/Cb Pr/Cr, also written as YCBCR or Y′CBCR, is a
                    // family of color spaces used as a part of the color image pipeline in video
                    // and digital photography systems. Y′ is the luma component and CB and CR are
                    // the blue-difference and red-difference chroma components. Y′ (with prime) is
                    // distinguished from Y, which is luminance, meaning that light intensity is
                    // nonlinearly encoded based on gamma corrected RGB primaries.
                    //
                    // Y′CbCr color spaces are defined by a mathematical coordinate transformation
                    // from an associated RGB primaries and white point. If the underlying RGB
                    // color space is absolute, the Y′CbCr color space is an absolute color space
                    // as well; conversely, if the RGB space is ill-defined, so is Y′CbCr. The
                    // transformation is defined in equations 32, 33 in ITU-T H.273. Nevertheless
                    // that rule does not apply to P3-D65 primaries used by Netflix with
                    // BT.2020-NCL matrix, so that means matrix was not derived from primaries, but
                    // now Netflix allows BT.2020 primaries (since 2021).[1] The same happens with
                    // JPEG: it has BT.601 matrix derived from System M primaries, yet the
                    // primaries of most images are BT.709.
                    VideoFormat::NV12 => [
                        unsafe {
                            std::slice::from_raw_parts(
                                frame.data[0] as *const _,
                                frame.linesize[0] * frame.height as usize,
                            )
                        },
                        unsafe {
                            std::slice::from_raw_parts(
                                frame.data[1] as *const _,
                                frame.linesize[1] * frame.height as usize,
                            )
                        },
                        &[],
                    ],
                    VideoFormat::I420 => [
                        unsafe {
                            std::slice::from_raw_parts(
                                frame.data[0] as *const _,
                                frame.linesize[0] * frame.height as usize,
                            )
                        },
                        unsafe {
                            std::slice::from_raw_parts(
                                frame.data[1] as *const _,
                                frame.linesize[1] * frame.height as usize,
                            )
                        },
                        unsafe {
                            std::slice::from_raw_parts(
                                frame.data[2] as *const _,
                                frame.linesize[2] * frame.height as usize,
                            )
                        },
                    ],
                };

                let texture = Texture2DBuffer {
                    buffers: &buffers,
                    size: Size {
                        width: frame.width,
                        height: frame.height,
                    },
                };

                let texture = match frame.format {
                    VideoFormat::BGRA => Texture::Bgra(Texture2DResource::Buffer(texture)),
                    VideoFormat::RGBA => Texture::Rgba(Texture2DResource::Buffer(texture)),
                    VideoFormat::NV12 => Texture::Nv12(Texture2DResource::Buffer(texture)),
                    VideoFormat::I420 => Texture::I420(texture),
                };

                match self {
                    #[cfg(target_os = "windows")]
                    Self::Direct3D11(render) => render.submit(texture)?,
                    Self::WebGPU(render) => render.submit(texture)?,
                }
            }
        }

        Ok(())
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
pub struct Renderer<'a>(Mutex<VideoRender<'a>>, Mutex<AudioRender>);

impl<'a> Renderer<'a> {
    pub fn new<T: Into<SurfaceTarget<'a>>>(
        backend: GraphicsBackend,
        window: T,
        size: Size,
    ) -> Result<Self, RendererError> {
        Ok(Self(
            Mutex::new(VideoRender::new(backend, window, size)?),
            Mutex::new(AudioRender::new()?),
        ))
    }
}

impl<'a> AVFrameSink for Renderer<'a> {
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
