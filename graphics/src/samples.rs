use std::sync::Arc;

use smallvec::SmallVec;
use utils::{win32::ID3D11Texture2D, Size};

use wgpu::{
    include_wgsl, BindGroup, BindGroupDescriptor, BindGroupEntry, BindGroupLayout,
    BindGroupLayoutDescriptor, BindGroupLayoutEntry, BindingResource, BindingType, BlendState,
    ColorTargetState, ColorWrites, Device, Extent3d, FragmentState, ImageCopyTexture,
    ImageDataLayout, IndexFormat, MultisampleState, Origin3d, PipelineCompilationOptions,
    PipelineLayoutDescriptor, PrimitiveState, PrimitiveTopology, Queue, RenderPipeline,
    RenderPipelineDescriptor, SamplerBindingType, SamplerDescriptor, ShaderStages,
    Texture as WGPUTexture, TextureAspect, TextureDescriptor, TextureDimension, TextureFormat,
    TextureSampleType, TextureUsages, TextureView, TextureViewDescriptor, TextureViewDimension,
    VertexState,
};

use crate::{
    helper::win32::{create_texture_from_dx11_texture, FromDxgiResourceError},
    Vertex,
};

pub enum HardwareTexture<'a> {
    Dx11(&'a ID3D11Texture2D),
}

impl<'a> HardwareTexture<'a> {
    fn texture(&self, device: &Device) -> Result<WGPUTexture, FromDxgiResourceError> {
        match self {
            HardwareTexture::Dx11(dx11) => create_texture_from_dx11_texture(device, dx11),
        }
    }
}

pub struct SoftwareTexture<'a> {
    pub size: Size,
    pub buffers: &'a [&'a [u8]],
}

pub enum TextureResource<'a> {
    Texture(HardwareTexture<'a>),
    Buffer(SoftwareTexture<'a>),
}

impl<'a> TextureResource<'a> {
    fn texture(&self, device: &Device) -> Result<Option<WGPUTexture>, FromDxgiResourceError> {
        Ok(match self {
            TextureResource::Texture(texture) => texture.texture(device).ok(),
            _ => None,
        })
    }

    fn size(&self, device: &Device) -> Result<Size, FromDxgiResourceError> {
        Ok(match self {
            TextureResource::Texture(texture) => {
                let size = texture.texture(device)?.size();
                Size {
                    width: size.width,
                    height: size.height,
                }
            }
            TextureResource::Buffer(resource) => resource.size,
        })
    }
}

pub enum Texture<'a> {
    Rgba(TextureResource<'a>),
    Nv12(TextureResource<'a>),
}

impl<'a> Texture<'a> {
    fn texture(&self, device: &Device) -> Result<Option<WGPUTexture>, FromDxgiResourceError> {
        Ok(match self {
            Texture::Rgba(texture) | Texture::Nv12(texture) => texture.texture(device)?,
        })
    }
}

trait Texture2DSample {
    fn create_texture_descriptor(size: Size) -> impl IntoIterator<Item = (Size, TextureFormat)>;

    fn views_descriptors<'a>(
        &'a self,
        texture: Option<&'a WGPUTexture>,
    ) -> impl IntoIterator<Item = (&'a WGPUTexture, TextureFormat, TextureAspect)>;

    fn copy_buffer_descriptors<'a>(
        &self,
        buffers: &'a [&'a [u8]],
    ) -> impl IntoIterator<Item = (&'a [u8], &WGPUTexture, TextureAspect, Size)>;

    fn create(device: &Device, size: Size) -> impl Iterator<Item = WGPUTexture> {
        Self::create_texture_descriptor(size)
            .into_iter()
            .map(|(size, format)| {
                device.create_texture(&TextureDescriptor {
                    label: None,
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: TextureDimension::D2,
                    usage: TextureUsages::TEXTURE_BINDING,
                    view_formats: &[],
                    size: Extent3d {
                        depth_or_array_layers: 1,
                        width: size.width,
                        height: size.height,
                    },
                    format,
                })
            })
    }

    fn bind_group_layout(&self, device: &Device) -> BindGroupLayout {
        let mut entries: SmallVec<[BindGroupLayoutEntry; 5]> = SmallVec::with_capacity(5);
        for (i, _) in self.views_descriptors(None).into_iter().enumerate() {
            entries.push(BindGroupLayoutEntry {
                count: None,
                binding: i as u32,
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

    fn bind_group(
        &self,
        device: &Device,
        layout: &BindGroupLayout,
        texture: Option<&WGPUTexture>,
    ) -> BindGroup {
        let sampler = device.create_sampler(&SamplerDescriptor::default());

        let mut views: SmallVec<[TextureView; 5]> = SmallVec::with_capacity(5);
        for (texture, format, aspect) in self.views_descriptors(texture) {
            views.push(texture.create_view(&TextureViewDescriptor {
                dimension: Some(TextureViewDimension::D2),
                format: Some(format),
                aspect,
                ..Default::default()
            }));
        }

        let mut entries: SmallVec<[BindGroupEntry; 5]> = SmallVec::with_capacity(5);
        for (i, view) in views.iter().enumerate() {
            entries.push(BindGroupEntry {
                binding: i as u32,
                resource: BindingResource::TextureView(view),
            });
        }

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

    fn update(&self, queue: &Queue, resource: &SoftwareTexture) {
        for (buffer, texture, aspect, size) in self.copy_buffer_descriptors(resource.buffers) {
            queue.write_texture(
                ImageCopyTexture {
                    aspect,
                    texture,
                    mip_level: 0,
                    origin: Origin3d::ZERO,
                },
                buffer,
                ImageDataLayout {
                    offset: 0,
                    bytes_per_row: Some(size.width),
                    rows_per_image: Some(size.height),
                },
                Extent3d {
                    width: size.width,
                    height: size.height,
                    depth_or_array_layers: 1,
                },
            );
        }
    }
}

struct Rgba(WGPUTexture);

impl Rgba {
    fn new(device: &Device, size: Size) -> Self {
        Self(Self::create(device, size).next().unwrap())
    }
}

impl Texture2DSample for Rgba {
    fn create_texture_descriptor(size: Size) -> impl IntoIterator<Item = (Size, TextureFormat)> {
        [(size, TextureFormat::Rgba8Unorm)]
    }

    fn views_descriptors<'a>(
        &'a self,
        texture: Option<&'a WGPUTexture>,
    ) -> impl IntoIterator<Item = (&'a WGPUTexture, TextureFormat, TextureAspect)> {
        [(
            texture.unwrap_or_else(|| &self.0),
            TextureFormat::Rgba8Unorm,
            TextureAspect::All,
        )]
    }

    fn copy_buffer_descriptors<'a>(
        &self,
        buffers: &'a [&'a [u8]],
    ) -> impl IntoIterator<Item = (&'a [u8], &WGPUTexture, TextureAspect, Size)> {
        let size = self.0.size();
        [(
            buffers[0],
            &self.0,
            TextureAspect::All,
            Size {
                width: size.width,
                height: size.height,
            },
        )]
    }
}

struct Nv12(WGPUTexture);

impl Nv12 {
    fn new(device: &Device, size: Size) -> Self {
        Self(Self::create(device, size).next().unwrap())
    }
}

impl Texture2DSample for Nv12 {
    fn create_texture_descriptor(size: Size) -> impl IntoIterator<Item = (Size, TextureFormat)> {
        [(size, TextureFormat::NV12)]
    }

    fn views_descriptors<'a>(
        &'a self,
        texture: Option<&'a WGPUTexture>,
    ) -> impl IntoIterator<Item = (&'a WGPUTexture, TextureFormat, TextureAspect)> {
        let texture = texture.unwrap_or_else(|| &self.0);
        [
            (texture, TextureFormat::R8Unorm, TextureAspect::Plane0),
            (texture, TextureFormat::Rg8Unorm, TextureAspect::Plane1),
        ]
    }

    fn copy_buffer_descriptors<'a>(
        &self,
        buffers: &'a [&'a [u8]],
    ) -> impl IntoIterator<Item = (&'a [u8], &WGPUTexture, TextureAspect, Size)> {
        let size = {
            let size = self.0.size();
            Size {
                width: size.width,
                height: size.height,
            }
        };

        [
            (buffers[0], &self.0, TextureAspect::Plane0, size),
            (buffers[1], &self.0, TextureAspect::Plane1, size),
        ]
    }
}

enum Texture2DSourceSample {
    Rgba(Rgba),
    Nv12(Nv12),
}

pub struct Texture2DSource {
    device: Arc<Device>,
    queue: Arc<Queue>,
    pipeline: Option<RenderPipeline>,
    sample: Option<Texture2DSourceSample>,
    bind_group_layout: Option<BindGroupLayout>,
}

impl Texture2DSource {
    pub fn new(device: Arc<Device>, queue: Arc<Queue>) -> Self {
        Self {
            bind_group_layout: None,
            pipeline: None,
            sample: None,
            device,
            queue,
        }
    }

    pub fn get_view(
        &mut self,
        texture: Texture,
    ) -> Result<Option<(&RenderPipeline, BindGroup)>, FromDxgiResourceError> {
        if self.sample.is_none() {
            let sample = match &texture {
                Texture::Rgba(texture) => Texture2DSourceSample::Rgba(Rgba::new(
                    &self.device,
                    texture.size(&self.device)?,
                )),
                Texture::Nv12(texture) => Texture2DSourceSample::Nv12(Nv12::new(
                    &self.device,
                    texture.size(&self.device)?,
                )),
            };

            let bind_group_layout = match &sample {
                Texture2DSourceSample::Rgba(texture) => texture.bind_group_layout(&self.device),
                Texture2DSourceSample::Nv12(texture) => texture.bind_group_layout(&self.device),
            };

            let pipeline =
                self.device
                    .create_render_pipeline(&RenderPipelineDescriptor {
                        label: None,
                        layout: Some(&self.device.create_pipeline_layout(
                            &PipelineLayoutDescriptor {
                                label: None,
                                bind_group_layouts: &[&bind_group_layout],
                                push_constant_ranges: &[],
                            },
                        )),
                        vertex: VertexState {
                            entry_point: "main",
                            module: &self
                                .device
                                .create_shader_module(include_wgsl!("./shaders/vertex.wgsl")),
                            compilation_options: PipelineCompilationOptions::default(),
                            buffers: &[Vertex::desc()],
                        },
                        fragment: Some(FragmentState {
                            entry_point: "main",
                            module: &self.device.create_shader_module(match &sample {
                                Texture2DSourceSample::Rgba(_) => {
                                    include_wgsl!("./shaders/fragment/any.wgsl")
                                }
                                Texture2DSourceSample::Nv12(_) => {
                                    include_wgsl!("./shaders/fragment/nv12.wgsl")
                                }
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
                        multisample: MultisampleState::default(),
                        depth_stencil: None,
                        multiview: None,
                        cache: None,
                    });

            self.sample = Some(sample);
            self.pipeline = Some(pipeline);
            self.bind_group_layout = Some(bind_group_layout);
        }

        if let Some(sample) = &self.sample {
            match &texture {
                Texture::Rgba(TextureResource::Buffer(buffer)) => {
                    if let Texture2DSourceSample::Rgba(rgba) = sample {
                        rgba.update(&self.queue, buffer);
                    }
                }
                Texture::Nv12(TextureResource::Buffer(buffer)) => {
                    if let Texture2DSourceSample::Nv12(nv12) = sample {
                        nv12.update(&self.queue, buffer);
                    }
                }
                _ => (),
            }
        }

        Ok(
            if let (Some(layout), Some(sample), Some(pipeline)) =
                (&self.bind_group_layout, &self.sample, &self.pipeline)
            {
                let texture = texture.texture(&self.device)?;
                Some((
                    pipeline,
                    match sample {
                        Texture2DSourceSample::Rgba(sample) => {
                            sample.bind_group(&self.device, layout, texture.as_ref())
                        }
                        Texture2DSourceSample::Nv12(sample) => {
                            sample.bind_group(&self.device, layout, texture.as_ref())
                        }
                    },
                ))
            } else {
                None
            },
        )
    }
}
