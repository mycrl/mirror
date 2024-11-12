mod receiver;
mod sender;

pub use self::{
    receiver::{HylaranaReceiver, HylaranaReceiverDescriptor, HylaranaReceiverError},
    sender::{
        AudioDescriptor, HylaranaSender, HylaranaSenderDescriptor, HylaranaSenderError,
        HylaranaSenderSourceDescriptor, VideoDescriptor,
    },
};

use std::slice::from_raw_parts;

pub use hylarana_capture::{Capture, Source, SourceType};
pub use hylarana_codec::{VideoDecoderType, VideoEncoderType};
pub use hylarana_common::{
    frame::{AudioFrame, VideoFormat, VideoFrame, VideoSubFormat},
    Size,
};

pub use hylarana_graphics::raw_window_handle;
pub use hylarana_transport::TransportDescriptor;

#[cfg(target_os = "windows")]
use hylarana_common::win32::{
    d3d_texture_borrowed_raw, set_process_priority, shutdown as win32_shutdown,
    startup as win32_startup, windows::Win32::Foundation::HWND, Direct3DDevice, ProcessPriority,
};

#[cfg(target_os = "macos")]
use hylarana_common::macos::{CVPixelBufferRef, PixelBufferRef};

#[cfg(target_os = "windows")]
use parking_lot::RwLock;

#[cfg(target_os = "windows")]
use hylarana_graphics::dx11::Dx11Renderer;

use hylarana_graphics::{
    Renderer as WgpuRenderer, RendererOptions as WgpuRendererOptions, SurfaceTarget, Texture,
    Texture2DBuffer, Texture2DResource,
};

use parking_lot::Mutex;
use rodio::{OutputStream, OutputStreamHandle, Sink};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum HylaranaError {
    #[error(transparent)]
    #[cfg(target_os = "windows")]
    Win32Error(#[from] hylarana_common::win32::windows::core::Error),
    #[error(transparent)]
    TransportError(#[from] std::io::Error),
}

/// Initialize the environment, which must be initialized before using the sdk.
pub fn startup() -> Result<(), HylaranaError> {
    log::info!("hylarana startup");

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
    hylarana_capture::startup();

    hylarana_codec::startup();
    log::info!("codec initialized");

    hylarana_transport::startup();
    log::info!("transport initialized");

    log::info!("all initialized");
    Ok(())
}

/// Cleans up the environment when the sdk exits, and is recommended to be
/// called when the application exits.
pub fn shutdown() -> Result<(), HylaranaError> {
    log::info!("hylarana shutdown");

    hylarana_codec::shutdown();
    hylarana_transport::shutdown();

    #[cfg(target_os = "windows")]
    if let Err(e) = win32_shutdown() {
        log::warn!("{:?}", e);
    }

    Ok(())
}

pub trait AVFrameObserver: Sync + Send {
    /// Callback when the sender is closed. This may be because the external
    /// side actively calls the close, or the audio and video packets cannot be
    /// sent (the network is disconnected), etc.
    fn close(&self) {}
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

/// Abstraction of audio and video streams.
pub trait AVFrameStream: AVFrameSink + AVFrameObserver {}

pub struct Hylarana;

impl Hylarana {
    /// Create a sender, specify a bound NIC address, you can pass callback to
    /// get the device screen or sound callback, callback can be null, if it is
    /// null then it means no callback data is needed.
    pub fn create_sender<T: AVFrameStream + 'static>(
        options: HylaranaSenderDescriptor,
        sink: T,
    ) -> Result<HylaranaSender<T>, HylaranaSenderError> {
        log::info!("create sender: options={:?}", options);

        let sender = HylaranaSender::new(options.clone(), sink)?;
        log::info!("create sender done: id={:?}", sender.get_id());

        Ok(sender)
    }

    /// Create a receiver, specify a bound NIC address, you can pass callback to
    /// get the sender's screen or sound callback, callback can not be null.
    pub fn create_receiver<T: AVFrameStream + 'static>(
        id: String,
        options: HylaranaReceiverDescriptor,
        sink: T,
    ) -> Result<HylaranaReceiver<T>, HylaranaReceiverError> {
        log::info!("create receiver: id={:?}, options={:?}", id, options);

        HylaranaReceiver::new(id, options.clone(), sink)
    }
}

#[cfg(target_os = "windows")]
static DIRECT_3D_DEVICE: RwLock<Option<Direct3DDevice>> = RwLock::new(None);

// Check if the D3D device has been created. If not, create a global one.
#[cfg(target_os = "windows")]
pub(crate) fn get_direct3d() -> Direct3DDevice {
    if DIRECT_3D_DEVICE.read().is_none() {
        DIRECT_3D_DEVICE
            .write()
            .replace(Direct3DDevice::new().expect("D3D device was not initialized successfully!"));
    }

    DIRECT_3D_DEVICE.read().as_ref().unwrap().clone()
}

#[derive(Debug, Error)]
pub enum RendererError {
    #[error("no output device available")]
    AudioNotFoundOutputDevice,
    #[error(transparent)]
    AudioStreamError(#[from] rodio::StreamError),
    #[error(transparent)]
    AudioPlayError(#[from] rodio::PlayError),
    #[error("send audio queue error")]
    AudioSendQueueError,
    #[error(transparent)]
    #[cfg(target_os = "windows")]
    VideoDx11GraphicsError(#[from] hylarana_graphics::dx11::Dx11GraphicsError),
    #[error(transparent)]
    VideoGraphicsError(#[from] hylarana_graphics::GraphicsError),
    #[error("invalid d3d11texture2d texture")]
    #[cfg(target_os = "windows")]
    VideoInvalidD3D11Texture,
}

struct AudioSamples {
    sample_rate: u32,
    buffer: Vec<i16>,
    index: usize,
    frames: usize,
}

impl rodio::Source for AudioSamples {
    fn current_frame_len(&self) -> Option<usize> {
        Some(self.frames)
    }

    fn channels(&self) -> u16 {
        1
    }

    fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    fn total_duration(&self) -> Option<std::time::Duration> {
        None
    }
}

impl Iterator for AudioSamples {
    type Item = i16;

    fn next(&mut self) -> Option<Self::Item> {
        let item = self.buffer.get(self.index).map(|it| *it);
        self.index += 1;
        item
    }
}

impl From<&AudioFrame> for AudioSamples {
    fn from(frame: &AudioFrame) -> Self {
        Self {
            buffer: unsafe { from_raw_parts(frame.data, frame.frames as usize) }.to_vec(),
            sample_rate: frame.sample_rate,
            frames: frame.frames as usize,
            index: 0,
        }
    }
}

/// Audio player that plays the original audio frames directly.
pub struct AudioRender {
    #[allow(dead_code)]
    stream: OutputStream,
    #[allow(dead_code)]
    stream_handle: OutputStreamHandle,
    sink: Sink,
}

unsafe impl Send for AudioRender {}
unsafe impl Sync for AudioRender {}

impl AudioRender {
    /// Create a video player.
    pub fn new() -> Result<Self, RendererError> {
        let (stream, stream_handle) = OutputStream::try_default()?;
        let sink = Sink::try_new(&stream_handle)?;

        sink.play();
        Ok(Self {
            stream_handle,
            stream,
            sink,
        })
    }

    /// Push an audio clip to the queue.
    pub fn send(&self, frame: &AudioFrame) -> Result<(), RendererError> {
        self.sink.append(AudioSamples::from(frame));
        Ok(())
    }
}

impl Drop for AudioRender {
    fn drop(&mut self) {
        self.sink.pause();
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
        let direct3d = get_direct3d();

        Ok(match backend {
            #[cfg(target_os = "windows")]
            GraphicsBackend::Direct3D11 => Self::Direct3D11(Dx11Renderer::new(
                match window.into() {
                    SurfaceTarget::Window(window) => match window.window_handle().unwrap().as_raw()
                    {
                        raw_window_handle::RawWindowHandle::Win32(window) => {
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
            #[allow(unreachable_patterns)]
            _ => unimplemented!("not supports the {:?} backend", backend),
        })
    }

    pub fn send(&mut self, frame: &VideoFrame) -> Result<(), RendererError> {
        match frame.sub_format {
            #[cfg(target_os = "windows")]
            VideoSubFormat::D3D11 => {
                let texture =
                    Texture2DResource::Texture(hylarana_graphics::Texture2DRaw::ID3D11Texture2D(
                        d3d_texture_borrowed_raw(&(frame.data[0] as *mut _))
                            .ok_or_else(|| RendererError::VideoInvalidD3D11Texture)?
                            .clone(),
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
            #[cfg(target_os = "macos")]
            VideoSubFormat::CvPixelBufferRef => {
                let pixel_buffer = PixelBufferRef::from(frame.data[0] as CVPixelBufferRef);
                let linesize = pixel_buffer.linesize();
                let data = pixel_buffer.data();
                let size = pixel_buffer.size();

                let buffers = [
                    unsafe {
                        from_raw_parts(data[0] as *const _, linesize[0] * size.height as usize)
                    },
                    unsafe {
                        from_raw_parts(data[1] as *const _, linesize[1] * size.height as usize)
                    },
                    &[],
                ];

                match self {
                    Self::WebGPU(render) => render.submit(Texture::Nv12(
                        Texture2DResource::Buffer(Texture2DBuffer {
                            buffers: &buffers,
                            size,
                        }),
                    ))?,
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
                            from_raw_parts(
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
                            from_raw_parts(
                                frame.data[0] as *const _,
                                frame.linesize[0] * frame.height as usize,
                            )
                        },
                        unsafe {
                            from_raw_parts(
                                frame.data[1] as *const _,
                                frame.linesize[1] * frame.height as usize,
                            )
                        },
                        &[],
                    ],
                    VideoFormat::I420 => [
                        unsafe {
                            from_raw_parts(
                                frame.data[0] as *const _,
                                frame.linesize[0] * frame.height as usize,
                            )
                        },
                        unsafe {
                            from_raw_parts(
                                frame.data[1] as *const _,
                                frame.linesize[1] * frame.height as usize,
                            )
                        },
                        unsafe {
                            from_raw_parts(
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
            #[allow(unreachable_patterns)]
            _ => unimplemented!("not suppports the frame format = {:?}", frame.sub_format),
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
pub struct Renderer<'a> {
    video: Mutex<VideoRender<'a>>,
    audio: AudioRender,
}

impl<'a> Renderer<'a> {
    pub fn new<T: Into<SurfaceTarget<'a>>>(
        backend: GraphicsBackend,
        window: T,
        size: Size,
    ) -> Result<Self, RendererError> {
        Ok(Self {
            video: Mutex::new(VideoRender::new(backend, window, size)?),
            audio: AudioRender::new()?,
        })
    }
}

impl<'a> AVFrameSink for Renderer<'a> {
    /// Renders the audio frame, note that a queue is maintained internally,
    /// here it just pushes the audio to the playback queue, and if the queue is
    /// empty, it fills the mute data to the player by default, so you need to
    /// pay attention to the push rate.
    fn audio(&self, frame: &AudioFrame) -> bool {
        if let Err(e) = self.audio.send(frame) {
            log::error!("{:?}", e);

            return false;
        }

        true
    }

    /// Renders video frames and can automatically handle rendering of hardware
    /// textures and rendering textures.
    fn video(&self, frame: &VideoFrame) -> bool {
        if let Err(e) = self.video.lock().send(frame) {
            log::error!("{:?}", e);

            return false;
        }

        true
    }
}
