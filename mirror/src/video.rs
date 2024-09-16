#[cfg(target_os = "windows")]
use std::ptr::null_mut;

use crate::Window;

use anyhow::{anyhow, Result};
use bytemuck::{Pod, Zeroable};
use frame::{VideoFormat, VideoFrame};
use pollster::FutureExt;

#[cfg(target_os = "windows")]
use utils::win32::{
    d3d_texture_borrowed_raw, ID3D11Texture2D, ID3D12Resource, Interface, SharedTexture,
};

use utils::Size;
#[cfg(target_os = "windows")]
use wgpu::hal::api::Dx12;

use wgpu::{
    include_wgsl, util::{BufferInitDescriptor, DeviceExt}, BindGroup, BindGroupDescriptor, BindGroupEntry, BindGroupLayout, BindGroupLayoutDescriptor, BindGroupLayoutEntry, BindingResource, BindingType, BlendState, Buffer, BufferAddress, BufferUsages, Color, ColorTargetState, ColorWrites, CommandEncoder, CommandEncoderDescriptor, Device, Extent3d, Features, FragmentState, ImageCopyTexture, ImageCopyTextureBase, IndexFormat, Instance, Limits, LoadOp, MemoryHints, MultisampleState, Operations, Origin3d, PipelineCompilationOptions, PipelineLayoutDescriptor, PresentMode, PrimitiveState, PrimitiveTopology, Queue, RenderPassColorAttachment, RenderPassDescriptor, RenderPipeline, RenderPipelineDescriptor, RequestAdapterOptions, SamplerBindingType, SamplerDescriptor, ShaderStages, StoreOp, Surface, Texture, TextureAspect, TextureDescriptor, TextureDimension, TextureFormat, TextureSampleType, TextureUsages, TextureViewDescriptor, TextureViewDimension, VertexAttribute, VertexBufferLayout, VertexFormat, VertexState, VertexStepMode
};

pub struct VideoPlayer {
    surface: Surface<'static>,
    device: Device,
    queue: Queue,
    vertex_buffer: Buffer,
    index_buffer: Buffer,
    pipeline: Option<RenderPipeline>,
    bind_group: Option<BindGroup>,
    texture: Option<InputTexture>,
}

impl VideoPlayer {
    pub fn new(window: Window) -> Result<Self> {
        let window_size = window.size()?;
        let instance = Instance::default();
        let surface = instance.create_surface(window.clone())?;
        let adapter = instance
            .request_adapter(&RequestAdapterOptions {
                compatible_surface: Some(&surface),
                ..Default::default()
            })
            .block_on()
            .ok_or_else(|| anyhow!("Failed to find an appropriate adapter"))?;

        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: None,
                    required_features: Features::TEXTURE_COMPRESSION_BC
                        | Features::TEXTURE_FORMAT_NV12,
                    required_limits: Limits::downlevel_defaults(),
                    memory_hints: MemoryHints::MemoryUsage,
                },
                None,
            )
            .block_on()?;

        {
            let mut config = surface
                .get_default_config(&adapter, window_size.width, window_size.height)
                .ok_or_else(|| anyhow!("Failed to find an surface default config"))?;

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
            surface,
            device,
            queue,
            vertex_buffer,
            index_buffer,
            bind_group: None,
            pipeline: None,
            texture: None,
        })
    }

    pub fn send(&mut self, frame: &VideoFrame) -> Result<()> {
        if self.texture.is_none() {
            self.texture = Some(InputTexture::new(
                Size {
                    width: frame.width,
                    height: frame.height,
                },
                frame.format,
                &self.device,
            ));
        }

        if self.bind_group.is_none() {
            if let Some(texture) = &self.texture {
                let layout = texture.bind_group_layout(&self.device);
                self.bind_group = Some(texture.bind_group(&layout, &self.device));
                self.pipeline = Some(self.device.create_render_pipeline(
                    &RenderPipelineDescriptor {
                        label: None,
                        layout: Some(&self.device.create_pipeline_layout(
                            &PipelineLayoutDescriptor {
                                label: None,
                                bind_group_layouts: &[&layout],
                                push_constant_ranges: &[],
                            },
                        )),
                        vertex:
                            VertexState {
                                entry_point: "main",
                                module:
                                    &self.device.create_shader_module(include_wgsl!(
                                        "./shaders/vertex.wgsl"
                                    )),
                                compilation_options: PipelineCompilationOptions::default(),
                                buffers: &[Vertex::desc()],
                            },
                        fragment: Some(FragmentState {
                            entry_point: "main",
                            module: &self.device.create_shader_module(match frame.format {
                                VideoFormat::RGBA => include_wgsl!("./shaders/fragment.wgsl"),
                                VideoFormat::NV12 => include_wgsl!("./shaders/nv12_fragment.wgsl"),
                                VideoFormat::I420 => todo!(),
                            }),
                            compilation_options: PipelineCompilationOptions::default(),
                            targets: &[Some(ColorTargetState {
                                blend: Some(BlendState::REPLACE),
                                write_mask: ColorWrites::ALL,
                                format: TextureFormat::Rgba8Unorm,
                            })],
                        }),
                        primitive: PrimitiveState {
                            topology: PrimitiveTopology::TriangleStrip,
                            strip_index_format: Some(IndexFormat::Uint16),
                            ..Default::default()
                        },
                        depth_stencil: None,
                        multisample: MultisampleState::default(),
                        multiview: None,
                        cache: None,
                    },
                ));
            }
        }

        if let (Some(bind_group), Some(pipeline)) = (&self.bind_group, &self.pipeline) {
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
                render_pass.set_bind_group(0, bind_group, &[]);
                render_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
                render_pass.set_index_buffer(self.index_buffer.slice(..), IndexFormat::Uint16);
                render_pass.draw_indexed(0..Vertex::INDICES.len() as u32, 0, 0..1);
            }

            self.queue.submit(Some(encoder.finish()));
            output.present();
        }

        Ok(())
    }

    fn initialize_pipeline(&mut self) {
        
    }
}

#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
struct Vertex {
    position: [f32; 2],
    tex_coords: [f32; 2],
}

impl Vertex {
    const INDICES: &'static [u16] = &[0, 1, 2, 2, 1, 3];

    const VERTICES: &'static [Vertex] = &[
        Vertex::new([-1.0, -1.0], [0.0, 0.0]),
        Vertex::new([1.0, -1.0], [1.0, 0.0]),
        Vertex::new([-1.0, 1.0], [0.0, 1.0]),
        Vertex::new([1.0, 1.0], [1.0, 1.0]),
    ];

    const fn new(position: [f32; 2], tex_coords: [f32; 2]) -> Self {
        Self {
            position,
            tex_coords,
        }
    }

    fn desc<'a>() -> VertexBufferLayout<'a> {
        VertexBufferLayout {
            array_stride: std::mem::size_of::<Self>() as BufferAddress,
            step_mode: VertexStepMode::Vertex,
            attributes: &[
                VertexAttribute {
                    offset: 0,
                    shader_location: 0,
                    format: VertexFormat::Float32x2,
                },
                VertexAttribute {
                    offset: std::mem::size_of::<[f32; 2]>() as BufferAddress,
                    shader_location: 1,
                    format: VertexFormat::Float32x2,
                },
            ],
        }
    }
}

#[cfg(target_os = "windows")]
pub fn create_texture_from_dx11_texture(
    device: &Device,
    texture: &ID3D11Texture2D,
    desc: &TextureDescriptor,
) -> Result<Texture> {
    let resource = unsafe {
        let handle = texture.get_shared()?;

        device
            .as_hal::<Dx12, _, _>(|hdevice| {
                hdevice.map(|hdevice| {
                    let raw_device = hdevice.raw_device();

                    let mut resource = null_mut();
                    if raw_device.OpenSharedHandle(
                        handle.0,
                        std::mem::transmute(&ID3D12Resource::IID),
                        &mut resource,
                    ) == 0
                    {
                        Some(resource)
                    } else {
                        None
                    }
                })
            })
            .ok_or_else(|| anyhow!("wgpu hal backend is not dx12"))?
            .ok_or_else(|| anyhow!("not found wgpu dx12 device"))?
            .ok_or_else(|| anyhow!("unable to open dx12 shared handle"))?
    };

    Ok(unsafe {
        let texture = <Dx12 as wgpu::hal::Api>::Device::texture_from_raw(
            d3d12::Resource::from_raw(resource as *mut _),
            desc.format,
            desc.dimension,
            desc.size,
            1,
            1,
        );

        device.create_texture_from_hal::<Dx12>(texture, &desc)
    })
}

enum InputTexture {
    Rgba(Texture),
    Nv12(Texture),
    I420(Texture, Texture, Texture),
}

impl InputTexture {
    fn new(size: Size, format: VideoFormat, device: &Device) -> Self {
        let options = TextureDescriptor {
            label: None,
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            usage: TextureUsages::TEXTURE_BINDING,
            format: match format {
                VideoFormat::RGBA => TextureFormat::Rgba8Unorm,
                VideoFormat::I420 => TextureFormat::R8Unorm,
                VideoFormat::NV12 => TextureFormat::NV12,
            },
            view_formats: &[],
            size: Extent3d {
                width: size.width,
                height: size.height,
                depth_or_array_layers: 1,
            },
        };

        match format {
            VideoFormat::RGBA => Self::Rgba(device.create_texture(&options)),
            VideoFormat::NV12 => Self::Nv12(device.create_texture(&options)),
            VideoFormat::I420 => Self::I420(
                device.create_texture(&options),
                device.create_texture(&TextureDescriptor {
                    size: Extent3d {
                        width: size.width / 2,
                        height: size.height / 2,
                        depth_or_array_layers: 1,
                    },
                    ..options
                }),
                device.create_texture(&TextureDescriptor {
                    size: Extent3d {
                        width: size.width / 2,
                        height: size.height / 2,
                        depth_or_array_layers: 1,
                    },
                    ..options
                }),
            ),
        }
    }

    fn bind_group_layout(&self, device: &Device) -> BindGroupLayout {
        let mut entries = Vec::new();

        for binding in 0..match self {
            Self::Rgba(_) => 1,
            Self::Nv12(_) => 2,
            Self::I420(_, _, _) => 3,
        } {
            entries.push(BindGroupLayoutEntry {
                binding,
                count: None,
                visibility: ShaderStages::FRAGMENT,
                ty: BindingType::Texture {
                    sample_type: TextureSampleType::Float { filterable: true },
                    view_dimension: TextureViewDimension::D2,
                    multisampled: false,
                },
            });
        }

        entries.push(BindGroupLayoutEntry {
            binding: entries.len() as u32,
            visibility: ShaderStages::FRAGMENT,
            ty: BindingType::Sampler(SamplerBindingType::Filtering),
            count: None,
        });

        device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: None,
            entries: &entries,
        })
    }

    fn bind_group(&self, layout: &BindGroupLayout, device: &Device) -> BindGroup {
        let views = match self {
            Self::Rgba(t) => vec![(t, TextureFormat::Rgba8Unorm, TextureAspect::All)],
            Self::Nv12(t) => vec![
                (t, TextureFormat::R8Unorm, TextureAspect::Plane0),
                (t, TextureFormat::Rg8Unorm, TextureAspect::Plane1),
            ],
            Self::I420(y, u, v) => vec![
                (y, TextureFormat::R8Unorm, TextureAspect::All),
                (u, TextureFormat::R8Unorm, TextureAspect::All),
                (v, TextureFormat::R8Unorm, TextureAspect::All),
            ],
        }
        .iter()
        .map(|(texture, format, aspect)| {
            texture.create_view(&TextureViewDescriptor {
                dimension: Some(TextureViewDimension::D2),
                format: Some(*format),
                aspect: *aspect,
                ..Default::default()
            })
        }).collect::<Vec<_>>();

        let mut entries = Vec::new();
        for (i, view) in views.iter().enumerate() {
            entries.push(BindGroupEntry {
                binding: i as u32,
                resource: BindingResource::TextureView(view),
            });
        }

        let sampler = device.create_sampler(&SamplerDescriptor::default());
        entries.push(BindGroupEntry {
            binding: entries.len() as u32,
            resource: BindingResource::Sampler(&sampler),
        });

        device.create_bind_group(&BindGroupDescriptor {
            label: None,
            entries: &entries,
            layout,
        })
    }

    fn update(&self, encoder: &mut CommandEncoder, texture: &Texture) {
        match self {
            Self::I420(_, _, _) => unreachable!(),
            Self::Rgba(dest) | Self::Nv12(dest) => {
                encoder.copy_texture_to_texture(ImageCopyTexture {
                    texture,
                    mip_level: 1,
                    origin: Origin3d::ZERO,
                    aspect: TextureAspect::All,
                }, ImageCopyTexture {
                    texture: dest,
                    mip_level: 1,
                    origin: Origin3d::ZERO,
                    aspect: TextureAspect::All,
                }, texture.size())
            },
        }
    }
}
