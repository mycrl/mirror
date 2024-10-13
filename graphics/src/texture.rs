mod bgra;
mod i420;
mod nv12;
mod rgba;

use std::sync::Arc;

use self::{bgra::Bgra, i420::I420, nv12::Nv12, rgba::Rgba};
use crate::{helper::CompatibilityLayerError, Vertex};

#[cfg(target_os = "windows")]
use crate::helper::win32::Dx11OnWgpuCompatibilityLayer as CompatibilityLayer;

#[cfg(any(target_os = "linux", target_os = "macos"))]
use crate::helper::any::EmptyOnWgpuCompatibilityLayer as CompatibilityLayer;

use common::Size;
use smallvec::SmallVec;
use thiserror::Error;

#[cfg(target_os = "windows")]
use common::win32::{
    windows::Win32::Graphics::Direct3D11::ID3D11Texture2D, Direct3DDevice, EasyTexture,
};

use wgpu::{
    include_wgsl, AddressMode, BindGroup, BindGroupDescriptor, BindGroupEntry, BindGroupLayout,
    BindGroupLayoutDescriptor, BindGroupLayoutEntry, BindingResource, BindingType, BlendState,
    ColorTargetState, ColorWrites, Device, Extent3d, FilterMode, FragmentState, ImageCopyTexture,
    ImageDataLayout, IndexFormat, MultisampleState, Origin3d, PipelineCompilationOptions,
    PipelineLayoutDescriptor, PrimitiveState, PrimitiveTopology, Queue, RenderPipeline,
    RenderPipelineDescriptor, SamplerBindingType, SamplerDescriptor, ShaderModuleDescriptor,
    ShaderStages, Texture as WGPUTexture, TextureAspect, TextureDescriptor, TextureDimension,
    TextureFormat, TextureSampleType, TextureUsages, TextureView, TextureViewDescriptor,
    TextureViewDimension, VertexState,
};

#[derive(Debug, Error)]
pub enum FromNativeResourceError {
    #[error(transparent)]
    CompatibilityLayerError(#[from] CompatibilityLayerError),
}

#[derive(Debug)]
pub enum Texture2DRaw {
    #[cfg(target_os = "windows")]
    ID3D11Texture2D(ID3D11Texture2D, u32),
}

impl Texture2DRaw {
    #[cfg(target_os = "windows")]
    pub(crate) fn texture<'b>(
        &self,
        compatibility: &'b mut CompatibilityLayer,
    ) -> Result<&'b WGPUTexture, FromNativeResourceError> {
        Ok(match self {
            Self::ID3D11Texture2D(dx11, index) => compatibility.from_hal(dx11, *index)?,
        })
    }

    #[cfg(target_os = "windows")]
    pub(crate) fn size(&self) -> Size {
        match self {
            Self::ID3D11Texture2D(dx11, _) => {
                let desc = dx11.desc();
                Size {
                    width: desc.Width,
                    height: desc.Height,
                }
            }
        }
    }
}

#[derive(Debug)]
pub struct Texture2DBuffer<'a> {
    pub size: Size,
    pub buffers: &'a [&'a [u8]],
}

#[derive(Debug)]
pub enum Texture2DResource<'a> {
    #[cfg(target_os = "windows")]
    Texture(Texture2DRaw),
    Buffer(Texture2DBuffer<'a>),
}

impl<'a> Texture2DResource<'a> {
    /// Get the hardware texture, here does not deal with software texture, so
    /// if it is software texture directly return None.
    #[allow(unused_variables)]
    pub(crate) fn texture<'b>(
        &self,
        compatibility: &'b mut CompatibilityLayer,
    ) -> Result<Option<&'b WGPUTexture>, FromNativeResourceError> {
        Ok(match self {
            #[cfg(target_os = "windows")]
            Texture2DResource::Texture(texture) => Some(texture.texture(compatibility)?),
            Texture2DResource::Buffer(_) => None,
        })
    }

    pub(crate) fn size(&self) -> Size {
        match self {
            #[cfg(target_os = "windows")]
            Texture2DResource::Texture(texture) => texture.size(),
            Texture2DResource::Buffer(texture) => texture.size,
        }
    }
}

#[derive(Debug)]
pub enum Texture<'a> {
    Bgra(Texture2DResource<'a>),
    Rgba(Texture2DResource<'a>),
    Nv12(Texture2DResource<'a>),
    I420(Texture2DBuffer<'a>),
}

impl<'a> Texture<'a> {
    pub(crate) fn texture<'b>(
        &self,
        compatibility: &'b mut CompatibilityLayer,
    ) -> Result<Option<&'b WGPUTexture>, FromNativeResourceError> {
        Ok(match self {
            Texture::Rgba(texture) | Texture::Bgra(texture) | Texture::Nv12(texture) => {
                texture.texture(compatibility)?
            }
            Texture::I420(_) => None,
        })
    }

    pub(crate) fn size(&self) -> Size {
        match self {
            Texture::Rgba(texture) | Texture::Bgra(texture) | Texture::Nv12(texture) => {
                texture.size()
            }
            Texture::I420(texture) => texture.size,
        }
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
                    // The textures created here are all needed to allow external writing of data,
                    // and all need the COPY_DST flag.
                    usage: TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST,
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

    /// Creates a new BindGroupLayout.
    ///
    /// A BindGroupLayout is a handle to the GPU-side layout of a binding group.
    /// It can be used to create a BindGroupDescriptor object, which in turn can
    /// be used to create a BindGroup object with Device::create_bind_group. A
    /// series of BindGroupLayouts can also be used to create a
    /// PipelineLayoutDescriptor, which can be used to create a PipelineLayout.
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

    /// Creates a new BindGroup.
    ///
    /// A BindGroup represents the set of resources bound to the bindings
    /// described by a BindGroupLayout. It can be created with
    /// Device::create_bind_group. A BindGroup can be bound to a particular
    /// RenderPass with RenderPass::set_bind_group, or to a ComputePass with
    /// ComputePass::set_bind_group.
    fn bind_group(
        &self,
        device: &Device,
        layout: &BindGroupLayout,
        texture: Option<&WGPUTexture>,
    ) -> BindGroup {
        let sampler = device.create_sampler(&SamplerDescriptor {
            address_mode_u: AddressMode::ClampToEdge,
            address_mode_v: AddressMode::ClampToEdge,
            address_mode_w: AddressMode::ClampToEdge,
            mipmap_filter: FilterMode::Nearest,
            mag_filter: FilterMode::Nearest,
            min_filter: FilterMode::Nearest,
            ..Default::default()
        });

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

    /// Schedule a write of some data into a texture.
    fn update(&self, queue: &Queue, resource: &Texture2DBuffer) {
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
                    // Bytes per "row" in an image.
                    //
                    // A row is one row of pixels or of compressed blocks in the x direction.
                    bytes_per_row: Some(size.width),
                    rows_per_image: Some(size.height),
                },
                texture.size(),
            );
        }
    }
}

enum Texture2DSourceSample {
    Bgra(Bgra),
    Rgba(Rgba),
    Nv12(Nv12),
    I420(I420),
}

impl Texture2DSourceSample {
    fn from_texture(device: &Device, texture: &Texture, size: Size) -> Self {
        match texture {
            Texture::Bgra(_) => Self::Bgra(Bgra::new(device, size)),
            Texture::Rgba(_) => Self::Rgba(Rgba::new(device, size)),
            Texture::Nv12(_) => Self::Nv12(Nv12::new(device, size)),
            Texture::I420(_) => Self::I420(I420::new(device, size)),
        }
    }

    fn fragment(&self) -> ShaderModuleDescriptor {
        match self {
            Texture2DSourceSample::Rgba(_) => {
                include_wgsl!("./shaders/fragment/any.wgsl")
            }
            Texture2DSourceSample::Bgra(_) => {
                include_wgsl!("./shaders/fragment/any.wgsl")
            }
            Texture2DSourceSample::Nv12(_) => {
                include_wgsl!("./shaders/fragment/nv12.wgsl")
            }
            Texture2DSourceSample::I420(_) => {
                include_wgsl!("./shaders/fragment/i420.wgsl")
            }
        }
    }

    fn bind_group_layout(&self, device: &Device) -> BindGroupLayout {
        match self {
            Self::Bgra(texture) => texture.bind_group_layout(device),
            Self::Rgba(texture) => texture.bind_group_layout(device),
            Self::Nv12(texture) => texture.bind_group_layout(device),
            Self::I420(texture) => texture.bind_group_layout(device),
        }
    }
}

pub struct Texture2DSourceOptions {
    #[cfg(target_os = "windows")]
    pub direct3d: Direct3DDevice,
    pub device: Arc<Device>,
    pub queue: Arc<Queue>,
}

pub struct Texture2DSource {
    device: Arc<Device>,
    queue: Arc<Queue>,
    pipeline: Option<RenderPipeline>,
    sample: Option<Texture2DSourceSample>,
    bind_group_layout: Option<BindGroupLayout>,
    compatibility: CompatibilityLayer,
}

impl Texture2DSource {
    pub fn new(options: Texture2DSourceOptions) -> Result<Self, FromNativeResourceError> {
        #[cfg(target_os = "windows")]
        let compatibility = CompatibilityLayer::new(options.device.clone(), options.direct3d);

        #[cfg(any(target_os = "linux", target_os = "macos"))]
        let compatibility = CompatibilityLayer::new(options.device.clone());

        Ok(Self {
            device: options.device,
            queue: options.queue,
            bind_group_layout: None,
            pipeline: None,
            sample: None,
            compatibility,
        })
    }

    /// If it is a hardware texture, it will directly create view for the
    /// current texture, if it is a software texture, it will write the data to
    /// the internal texture first, and then create the view for the internal
    /// texture, so it is a more time-consuming operation to use the software
    /// texture.
    pub fn get_view(
        &mut self,
        texture: Texture,
    ) -> Result<Option<(&RenderPipeline, BindGroup)>, FromNativeResourceError> {
        // Not yet initialized, initialize the environment first.
        if self.sample.is_none() {
            let size = texture.size();
            let sample = Texture2DSourceSample::from_texture(&self.device, &texture, size);
            let bind_group_layout = sample.bind_group_layout(&self.device);

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
                            entry_point: Some("main"),
                            module: &self
                                .device
                                .create_shader_module(include_wgsl!("./shaders/vertex.wgsl")),
                            compilation_options: PipelineCompilationOptions::default(),
                            buffers: &[Vertex::desc()],
                        },
                        fragment: Some(FragmentState {
                            entry_point: Some("main"),
                            module: &self.device.create_shader_module(sample.fragment()),
                            compilation_options: PipelineCompilationOptions::default(),
                            targets: &[Some(ColorTargetState {
                                blend: Some(BlendState::REPLACE),
                                write_mask: ColorWrites::ALL,
                                format: TextureFormat::Bgra8Unorm,
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

        // Only software textures need to be updated to the sample via update.
        #[allow(unreachable_patterns)]
        if let Some(sample) = &self.sample {
            match &texture {
                Texture::Bgra(Texture2DResource::Buffer(buffer)) => {
                    if let Texture2DSourceSample::Bgra(rgba) = sample {
                        rgba.update(&self.queue, buffer);
                    }
                }
                Texture::Rgba(Texture2DResource::Buffer(buffer)) => {
                    if let Texture2DSourceSample::Rgba(rgba) = sample {
                        rgba.update(&self.queue, buffer);
                    }
                }
                Texture::Nv12(Texture2DResource::Buffer(buffer)) => {
                    if let Texture2DSourceSample::Nv12(nv12) = sample {
                        nv12.update(&self.queue, buffer);
                    }
                }
                Texture::I420(texture) => {
                    if let Texture2DSourceSample::I420(i420) = sample {
                        i420.update(&self.queue, texture);
                    }
                }
                _ => (),
            }
        }

        Ok(
            if let (Some(layout), Some(sample), Some(pipeline)) =
                (&self.bind_group_layout, &self.sample, &self.pipeline)
            {
                let texture = texture.texture(&mut self.compatibility)?;
                Some((
                    pipeline,
                    match sample {
                        Texture2DSourceSample::Bgra(sample) => {
                            sample.bind_group(&self.device, layout, texture)
                        }
                        Texture2DSourceSample::Rgba(sample) => {
                            sample.bind_group(&self.device, layout, texture)
                        }
                        Texture2DSourceSample::Nv12(sample) => {
                            sample.bind_group(&self.device, layout, texture)
                        }
                        Texture2DSourceSample::I420(sample) => {
                            sample.bind_group(&self.device, layout, texture)
                        }
                    },
                ))
            } else {
                None
            },
        )
    }
}
