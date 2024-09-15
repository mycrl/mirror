use anyhow::{anyhow, Result};
use bytemuck::{Pod, Zeroable};
use frame::VideoFrame;
use pollster::FutureExt;
use utils::win32::{ID3D11Device, ID3D11Texture2D, ID3D12Resource, IDXGIResource, Interface};
use wgpu::{
    hal::api::Dx12,
    include_wgsl,
    util::{BufferInitDescriptor, DeviceExt},
    BindGroup, BindGroupDescriptor, BindGroupEntry, BindGroupLayoutDescriptor,
    BindGroupLayoutEntry, BindingResource, BindingType, BlendState, Buffer, BufferAddress,
    BufferUsages, Color, ColorTargetState, ColorWrites, CommandEncoderDescriptor, Device, Extent3d,
    Features, FragmentState, IndexFormat, Instance, Limits, LoadOp, MemoryHints, MultisampleState,
    Operations, PipelineCompilationOptions, PipelineLayoutDescriptor, PresentMode, PrimitiveState,
    PrimitiveTopology, Queue, RenderPassColorAttachment, RenderPassDescriptor, RenderPipeline,
    RenderPipelineDescriptor, RequestAdapterOptions, SamplerBindingType, SamplerDescriptor,
    ShaderStages, StoreOp, Surface, Texture, TextureAspect, TextureDescriptor, TextureDimension,
    TextureFormat, TextureSampleType, TextureUsages, TextureViewDescriptor, TextureViewDimension,
    VertexAttribute, VertexBufferLayout, VertexFormat, VertexState, VertexStepMode,
};

use crate::Window;

pub struct VideoPlayer {
    surface: Surface<'static>,
    device: Device,
    queue: Queue,
    vertex_buffer: Buffer,
    index_buffer: Buffer,
    pipeline: Option<RenderPipeline>,
    bind_group: Option<BindGroup>,
    texture: Option<Texture>,
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
                    required_features: Features::empty(),
                    required_limits: Limits::default(),
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
            contents: bytemuck::cast_slice(VERTICES),
            usage: BufferUsages::VERTEX,
        });

        let index_buffer = device.create_buffer_init(&BufferInitDescriptor {
            label: None,
            contents: bytemuck::cast_slice(INDICES),
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
        // if self.texture.is_none() {
        //     self.texture = Some(self.device.create_texture(&TextureDescriptor {
        //         label: None,
        //         mip_level_count: 1,
        //         sample_count: 1,
        //         format: TextureFormat::Rgba8Unorm,
        //         dimension: TextureDimension::D2,
        //         usage: TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST,
        //         view_formats: &[],
        //         size: Extent3d {
        //             width: frame.width,
        //             height: frame.height,
        //             depth_or_array_layers: 1,
        //         },
        //     }));
        // }

        if self.bind_group.is_none() {
            if let Some(texture) = &self.texture {
                let layout = self
                    .device
                    .create_bind_group_layout(&BindGroupLayoutDescriptor {
                        label: None,
                        entries: &[
                            BindGroupLayoutEntry {
                                binding: 0,
                                count: None,
                                visibility: ShaderStages::FRAGMENT,
                                ty: BindingType::Texture {
                                    sample_type: TextureSampleType::Float { filterable: true },
                                    view_dimension: TextureViewDimension::D2,
                                    multisampled: false,
                                },
                            },
                            BindGroupLayoutEntry {
                                binding: 1,
                                count: None,
                                visibility: ShaderStages::FRAGMENT,
                                ty: BindingType::Sampler(SamplerBindingType::Filtering),
                            },
                        ],
                    });

                self.bind_group = Some(self.device.create_bind_group(&BindGroupDescriptor {
                    label: None,
                    layout: &layout,
                    entries: &[
                        BindGroupEntry {
                            binding: 0,
                            resource: BindingResource::TextureView(&texture.create_view(
                                &TextureViewDescriptor {
                                    dimension: Some(TextureViewDimension::D2),
                                    format: Some(texture.format()),
                                    aspect: TextureAspect::All,
                                    ..Default::default()
                                },
                            )),
                        },
                        BindGroupEntry {
                            binding: 1,
                            resource: BindingResource::Sampler(
                                &self.device.create_sampler(&SamplerDescriptor::default()),
                            ),
                        },
                    ],
                }));

                self.pipeline =
                    Some(
                        self.device
                            .create_render_pipeline(&RenderPipelineDescriptor {
                                label: None,
                                layout: Some(&self.device.create_pipeline_layout(
                                    &PipelineLayoutDescriptor {
                                        label: None,
                                        bind_group_layouts: &[&layout],
                                        push_constant_ranges: &[],
                                    },
                                )),
                                vertex: VertexState {
                                    entry_point: "main",
                                    module: &self.device.create_shader_module(include_wgsl!(
                                        "./shaders/vertex.wgsl"
                                    )),
                                    compilation_options: PipelineCompilationOptions::default(),
                                    buffers: &[Vertex::desc()],
                                },
                                fragment: Some(FragmentState {
                                    entry_point: "main",
                                    module: &self.device.create_shader_module(include_wgsl!(
                                        "./shaders/fragment.wgsl"
                                    )),
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
                            }),
                    );
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
                render_pass.draw_indexed(0..INDICES.len() as u32, 0, 0..1);
            }

            self.queue.submit(Some(encoder.finish()));
            output.present();
        }

        Ok(())
    }
}

#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
struct Vertex {
    position: [f32; 2],
    tex_coords: [f32; 2],
}

impl Vertex {
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

const VERTICES: &'static [Vertex] = &[
    Vertex::new([-1.0, -1.0], [0.0, 0.0]),
    Vertex::new([1.0, -1.0], [1.0, 0.0]),
    Vertex::new([-1.0, 1.0], [0.0, 1.0]),
    Vertex::new([1.0, 1.0], [1.0, 1.0]),
];

const INDICES: &'static [u16] = &[0, 1, 2, 2, 1, 3];

#[cfg(target_os = "windows")]
pub fn create_dx12_resource_from_d3d11_texture(device: &Device, texture: &ID3D11Texture2D) -> Option<ID3D12Resource> {
    use std::ptr::null_mut;

    unsafe {
        let handle = texture.cast::<IDXGIResource>().ok()?.GetSharedHandle().ok()?;
        Some(device.as_hal::<Dx12, _, _>(|hdevice| {
            hdevice.map(|hdevice| {
                let raw_device = hdevice.raw_device();

                let mut resource = null_mut();
                if raw_device.OpenSharedHandle(handle.0, std::mem::transmute(&ID3D12Resource::IID), &mut resource) == 0 {
                    Some(ID3D12Resource::from_raw_borrowed(&resource).unwrap().clone())
                } else {
                    None
                }
            })

        })???)
    }
}

#[cfg(target_os = "windows")]
pub fn create_texture_from_dx12_resource(device: &Device, resource: ID3D12Resource, desc: &TextureDescriptor) -> Texture {
    unsafe {
        let texture = <Dx12 as wgpu::hal::Api>::Device::texture_from_raw(resource, desc.format, desc.dimension, desc.size, 1, 1);
        device.create_texture_from_hal::<Dx12>(texture, &desc)
    }
}
