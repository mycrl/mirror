mod helper;
mod samples;
mod vertex;

use std::sync::Arc;

use self::{samples::FromNativeResourceError, vertex::Vertex};

pub use self::samples::{HardwareTexture, SoftwareTexture, Texture, TextureResource};

use pollster::FutureExt;
use samples::Texture2DSource;
use thiserror::Error;
use utils::Size;
use wgpu::{
    util::{BufferInitDescriptor, DeviceExt},
    Backends, Buffer, BufferUsages, Color, CommandEncoderDescriptor, Device, Features, IndexFormat,
    Instance, InstanceDescriptor, LoadOp, MemoryHints, Operations, PresentMode, Queue,
    RenderPassColorAttachment, RenderPassDescriptor, RequestAdapterOptions, StoreOp, Surface,
    SurfaceTarget, TextureFormat, TextureUsages,
};

pub use wgpu::rwh as raw_window_handle;

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
    pub fn new(window: impl Into<SurfaceTarget<'a>>, size: Size) -> Result<Self, GraphicsError> {
        let instance = Instance::new(InstanceDescriptor {
            backends: if cfg!(target_os = "windows") {
                Backends::DX12
            } else {
                Backends::VULKAN
            },
            ..Default::default()
        });

        let surface = instance.create_surface(window)?;
        let adapter = instance
            .request_adapter(&RequestAdapterOptions {
                compatible_surface: Some(&surface),
                ..Default::default()
            })
            .block_on()
            .ok_or_else(|| GraphicsError::NotFoundAdapter)?;

        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: None,
                    memory_hints: MemoryHints::MemoryUsage,
                    required_features: adapter.features() | Features::TEXTURE_FORMAT_NV12,
                    required_limits: adapter.limits(),
                },
                None,
            )
            .block_on()?;

        let device = Arc::new(device);
        let queue = Arc::new(queue);

        // Configure surface as RGBA, RGBA this format compatibility is the best, in
        // order to unnecessary trouble, directly fixed to RGBA is the best.
        {
            let mut config = surface
                .get_default_config(&adapter, size.width, size.height)
                .ok_or_else(|| GraphicsError::NotFoundSurfaceDefaultConfig)?;

            config.present_mode = PresentMode::Fifo;
            config.format = TextureFormat::Rgba8Unorm;
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
            source: Texture2DSource::new(device.clone(), queue.clone()),
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
                .create_view(&wgpu::TextureViewDescriptor::default());

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
                render_pass.set_bind_group(0, &bind_group, &[]);
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
