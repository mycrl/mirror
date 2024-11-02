mod interop;
mod texture;
mod vertex;

use std::sync::Arc;

use self::vertex::Vertex;

pub use self::texture::{
    FromNativeResourceError, Texture, Texture2DBuffer, Texture2DRaw, Texture2DResource,
};

use hylarana_common::Size;
use pollster::FutureExt;
use texture::{Texture2DSource, Texture2DSourceOptions};
use thiserror::Error;
use wgpu::{
    util::{BufferInitDescriptor, DeviceExt},
    Backends, Buffer, BufferUsages, Color, CommandEncoderDescriptor, CompositeAlphaMode, Device,
    DeviceDescriptor, IndexFormat, Instance, InstanceDescriptor, LoadOp, MemoryHints, Operations,
    PowerPreference, PresentMode, Queue, RenderPassColorAttachment, RenderPassDescriptor,
    RequestAdapterOptions, StoreOp, Surface, TextureFormat, TextureUsages, TextureViewDescriptor,
};

pub use wgpu::{rwh as raw_window_handle, SurfaceTarget};

#[derive(Debug, Error)]
pub enum GraphicsError {
    #[error("not found graphics adaper")]
    NotFoundAdapter,
    #[error("not found graphics surface default config")]
    NotFoundSurfaceDefaultConfig,
    #[error(transparent)]
    RequestDeviceError(#[from] wgpu::RequestDeviceError),
    #[error(transparent)]
    SurfaceGetTextureFailed(#[from] wgpu::SurfaceError),
    #[error(transparent)]
    CreateSurfaceError(#[from] wgpu::CreateSurfaceError),
    #[error(transparent)]
    FromNativeResourceError(#[from] FromNativeResourceError),
}

#[derive(Debug)]
pub struct RendererOptions<T> {
    #[cfg(target_os = "windows")]
    pub direct3d: hylarana_common::win32::Direct3DDevice,
    pub window: T,
    pub size: Size,
}

/// Window Renderer.
///
/// Supports rendering RGBA or NV12 hardware or software textures to system
/// native windows.
///
/// Note that the renderer uses a hardware implementation by default, i.e. it
/// uses the underlying GPU device, and the use of software devices is not
/// currently supported.
pub struct Renderer<'a> {
    surface: Surface<'a>,
    device: Arc<Device>,
    queue: Arc<Queue>,
    vertex_buffer: Buffer,
    index_buffer: Buffer,
    source: Texture2DSource,
}

impl<'a> Renderer<'a> {
    pub fn new<T: Into<SurfaceTarget<'a>>>(
        options: RendererOptions<T>,
    ) -> Result<Self, GraphicsError> {
        let instance = Instance::new(InstanceDescriptor {
            backends: if cfg!(target_os = "windows") {
                Backends::DX12
            } else if cfg!(target_os = "linux") {
                Backends::VULKAN
            } else {
                Backends::METAL
            },
            ..Default::default()
        });

        let surface = instance.create_surface(options.window)?;
        let adapter = instance
            .request_adapter(&RequestAdapterOptions {
                power_preference: PowerPreference::LowPower,
                force_fallback_adapter: false,
                compatible_surface: Some(&surface),
                ..Default::default()
            })
            .block_on()
            .ok_or_else(|| GraphicsError::NotFoundAdapter)?;

        let (device, queue) = adapter
            .request_device(
                &DeviceDescriptor {
                    label: None,
                    memory_hints: MemoryHints::Performance,
                    required_features: adapter.features(),
                    required_limits: adapter.limits(),
                },
                None,
            )
            .block_on()?;

        let device = Arc::new(device);
        let queue = Arc::new(queue);

        // Configure surface as BGRA, BGRA this format compatibility is the best, in
        // order to unnecessary trouble, directly fixed to BGRA is the best.
        {
            let mut config = surface
                .get_default_config(&adapter, options.size.width, options.size.height)
                .ok_or_else(|| GraphicsError::NotFoundSurfaceDefaultConfig)?;

            config.present_mode = if cfg!(target_os = "windows") {
                PresentMode::Mailbox
            } else if cfg!(target_os = "linux") {
                PresentMode::Fifo
            } else {
                PresentMode::Immediate
            };

            config.format = TextureFormat::Bgra8Unorm;
            config.alpha_mode = CompositeAlphaMode::Opaque;
            config.usage = TextureUsages::RENDER_ATTACHMENT;
            surface.configure(&device, &config);
        };

        let vertex_buffer = device.create_buffer_init(&BufferInitDescriptor {
            label: None,
            contents: bytemuck::cast_slice(Vertex::VERTICES),
            usage: BufferUsages::VERTEX,
        });

        let index_buffer = device.create_buffer_init(&BufferInitDescriptor {
            label: None,
            contents: bytemuck::cast_slice(Vertex::INDICES),
            usage: BufferUsages::INDEX,
        });

        Ok(Self {
            source: Texture2DSource::new(Texture2DSourceOptions {
                #[cfg(target_os = "windows")]
                direct3d: options.direct3d,
                device: device.clone(),
                queue: queue.clone(),
            })?,
            vertex_buffer,
            index_buffer,
            surface,
            device,
            queue,
        })
    }

    // Submit the texture to the renderer, it should be noted that the renderer will
    // not render this texture immediately, the processing flow will enter the
    // render queue and wait for the queue to automatically schedule the rendering
    // to the surface.
    pub fn submit(&mut self, texture: Texture) -> Result<(), GraphicsError> {
        if let Some((pipeline, bind_group)) = self.source.get_view(texture)? {
            let output = self.surface.get_current_texture()?;
            let view = output
                .texture
                .create_view(&TextureViewDescriptor::default());

            let mut encoder = self
                .device
                .create_command_encoder(&CommandEncoderDescriptor { label: None });

            {
                let mut render_pass = encoder.begin_render_pass(&RenderPassDescriptor {
                    color_attachments: &[Some(RenderPassColorAttachment {
                        view: &view,
                        resolve_target: None,
                        ops: Operations {
                            load: LoadOp::Clear(Color::BLACK),
                            store: StoreOp::Store,
                        },
                    })],
                    ..Default::default()
                });

                render_pass.set_pipeline(pipeline);
                render_pass.set_bind_group(0, Some(&bind_group), &[]);
                render_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
                render_pass.set_index_buffer(self.index_buffer.slice(..), IndexFormat::Uint16);
                render_pass.draw_indexed(0..Vertex::INDICES.len() as u32, 0, 0..1);
            }

            self.queue.submit(Some(encoder.finish()));
            output.present();
        }

        Ok(())
    }
}

#[cfg(target_os = "windows")]
pub mod dx11 {
    use hylarana_common::{
        win32::{
            windows::Win32::{
                Foundation::HWND,
                Graphics::{
                    Direct3D11::{ID3D11RenderTargetView, ID3D11Texture2D, D3D11_VIEWPORT},
                    Dxgi::{
                        Common::{DXGI_FORMAT_NV12, DXGI_FORMAT_R8G8B8A8_UNORM},
                        CreateDXGIFactory, IDXGIFactory, IDXGISwapChain, DXGI_PRESENT,
                        DXGI_SWAP_CHAIN_DESC, DXGI_USAGE_RENDER_TARGET_OUTPUT,
                    },
                },
            },
            Direct3DDevice,
        },
        Size,
    };

    use hylarana_resample::win32::{Resource, VideoResampler, VideoResamplerDescriptor};
    use thiserror::Error;

    use crate::{Texture, Texture2DRaw, Texture2DResource};

    #[derive(Debug, Error)]
    pub enum Dx11GraphicsError {
        #[error(transparent)]
        WindowsError(#[from] hylarana_common::win32::windows::core::Error),
    }

    pub struct Dx11Renderer {
        direct3d: Direct3DDevice,
        swap_chain: IDXGISwapChain,
        render_target_view: ID3D11RenderTargetView,
        video_processor: Option<VideoResampler>,
    }

    unsafe impl Send for Dx11Renderer {}
    unsafe impl Sync for Dx11Renderer {}

    impl Dx11Renderer {
        pub fn new(
            window: HWND,
            size: Size,
            direct3d: Direct3DDevice,
        ) -> Result<Self, Dx11GraphicsError> {
            let swap_chain = unsafe {
                let dxgi_factory = CreateDXGIFactory::<IDXGIFactory>()?;

                let mut desc = DXGI_SWAP_CHAIN_DESC::default();
                desc.BufferCount = 1;
                desc.BufferDesc.Width = size.width;
                desc.BufferDesc.Height = size.height;
                desc.BufferDesc.Format = DXGI_FORMAT_R8G8B8A8_UNORM;
                desc.BufferUsage = DXGI_USAGE_RENDER_TARGET_OUTPUT;
                desc.OutputWindow = window;
                desc.SampleDesc.Count = 1;
                desc.Windowed = true.into();

                let mut swap_chain = None;
                dxgi_factory
                    .CreateSwapChain(&direct3d.device, &desc, &mut swap_chain)
                    .ok()?;

                swap_chain.unwrap()
            };

            let back_buffer = unsafe { swap_chain.GetBuffer::<ID3D11Texture2D>(0)? };
            let render_target_view = unsafe {
                let mut render_target_view = None;
                direct3d.device.CreateRenderTargetView(
                    &back_buffer,
                    None,
                    Some(&mut render_target_view),
                )?;

                render_target_view.unwrap()
            };

            unsafe {
                direct3d
                    .context
                    .OMSetRenderTargets(Some(&[Some(render_target_view.clone())]), None);
            }

            unsafe {
                let mut vp = D3D11_VIEWPORT::default();
                vp.Width = size.width as f32;
                vp.Height = size.height as f32;
                vp.MinDepth = 0.0;
                vp.MaxDepth = 1.0;

                direct3d.context.RSSetViewports(Some(&[vp]));
            }

            Ok(Self {
                video_processor: None,
                render_target_view,
                swap_chain,
                direct3d,
            })
        }

        /// Draw this pixel buffer to the configured SurfaceTexture.
        pub fn submit(&mut self, texture: Texture) -> Result<(), Dx11GraphicsError> {
            unsafe {
                self.direct3d
                    .context
                    .ClearRenderTargetView(&self.render_target_view, &[0.0, 0.0, 0.0, 1.0]);
            }

            if self.video_processor.is_none() {
                let size = texture.size();
                let format = match texture {
                    Texture::Nv12(_) => DXGI_FORMAT_NV12,
                    Texture::Rgba(_) => DXGI_FORMAT_R8G8B8A8_UNORM,
                    _ => unimplemented!("not supports texture format"),
                };

                self.video_processor
                    .replace(VideoResampler::new(VideoResamplerDescriptor {
                        direct3d: self.direct3d.clone(),
                        input: Resource::Default(format, size),
                        output: Resource::Texture(unsafe {
                            self.swap_chain.GetBuffer::<ID3D11Texture2D>(0)?
                        }),
                    })?);
            }

            if let Some(processor) = self.video_processor.as_mut() {
                let texture = match texture {
                    Texture::Rgba(texture) | Texture::Nv12(texture) => texture,
                    _ => unimplemented!("not supports texture format"),
                };

                let view = match texture {
                    Texture2DResource::Texture(texture) => match texture {
                        Texture2DRaw::ID3D11Texture2D(texture, index) => {
                            Some(processor.create_input_view(&texture, index)?)
                        }
                    },
                    Texture2DResource::Buffer(texture) => {
                        processor.update_input_from_buffer(
                            texture.buffers[0].as_ptr(),
                            texture.size.width,
                        )?;

                        None
                    }
                };

                processor.process(view)?;
            }

            unsafe {
                self.swap_chain.Present(0, DXGI_PRESENT(0)).ok()?;
            }

            Ok(())
        }
    }
}
