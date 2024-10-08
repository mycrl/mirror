use std::{
    slice::from_raw_parts,
    sync::{
        mpsc::{channel, Receiver, Sender},
        Arc,
    },
};

use parking_lot::RwLock;
use resample::AudioResampler;
use thiserror::Error;

use cpal::{
    traits::{DeviceTrait, HostTrait, StreamTrait},
    Stream, StreamConfig, StreamError,
};

use common::{
    frame::{AudioFrame, VideoFormat, VideoFrame, VideoSubFormat},
    Size,
};

use graphics::{
    Renderer, RendererOptions, SurfaceTarget, Texture, Texture2DBuffer, Texture2DResource,
};

#[cfg(target_os = "windows")]
use common::win32::{d3d_texture_borrowed_raw, windows::Win32::Foundation::HWND};

#[cfg(target_os = "windows")]
use graphics::dx11::Dx11Renderer;

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
    queue: Sender<Vec<i16>>,
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
    queue: Receiver<Vec<i16>>,
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
pub enum Backend {
    Dx11,
    Wgpu,
}

pub enum VideoRender<'a> {
    Wgpu(Renderer<'a>),
    #[cfg(target_os = "windows")]
    Dx11(Dx11Renderer),
}

impl<'a> VideoRender<'a> {
    pub fn new<T: Into<SurfaceTarget<'a>>>(
        backend: Backend,
        window: T,
        size: Size,
    ) -> Result<Self, RendererError> {
        log::info!(
            "create video player, backend={:?}, size={:?}",
            backend,
            size
        );

        #[cfg(target_os = "windows")]
        let direct3d = crate::DIRECT_3D_DEVICE.read().as_ref().unwrap().clone();

        Ok(match backend {
            #[cfg(not(target_os = "windows"))]
            Backend::Dx11 => unimplemented!("not supports dx11 backend"),
            #[cfg(target_os = "windows")]
            Backend::Dx11 => Self::Dx11(Dx11Renderer::new(
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
            Backend::Wgpu => Self::Wgpu(Renderer::new(RendererOptions {
                #[cfg(target_os = "windows")]
                direct3d,
                window,
                size,
            })?),
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

                    let texture = Texture2DResource::Texture(graphics::Texture2DRaw::Dx11(
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
                        Self::Dx11(render) => render.submit(texture)?,
                        Self::Wgpu(render) => render.submit(texture)?,
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
                    Self::Dx11(render) => render.submit(texture)?,
                    Self::Wgpu(render) => render.submit(texture)?,
                }
            }
        }

        Ok(())
    }
}
