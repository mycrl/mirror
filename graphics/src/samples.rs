use std::sync::Arc;

use crate::Vertex;

#[cfg(target_os = "windows")]
use crate::helper::win32::{create_texture_from_dx11_texture, FromDxgiResourceError};

use smallvec::SmallVec;
use thiserror::Error;
use utils::Size;

#[cfg(target_os = "windows")]
use utils::win32::windows::Win32::Graphics::Direct3D11::{ID3D11Texture2D, D3D11_TEXTURE2D_DESC};

use wgpu::{
    include_wgsl, AddressMode, BindGroup, BindGroupDescriptor, BindGroupEntry, BindGroupLayout,
    BindGroupLayoutDescriptor, BindGroupLayoutEntry, BindingResource, BindingType, BlendState,
    ColorTargetState, ColorWrites, Device, Extent3d, FilterMode, FragmentState, ImageCopyTexture,
    ImageDataLayout, IndexFormat, MultisampleState, Origin3d, PipelineCompilationOptions,
    PipelineLayoutDescriptor, PrimitiveState, PrimitiveTopology, Queue, RenderPipeline,
    RenderPipelineDescriptor, SamplerBindingType, SamplerDescriptor, ShaderStages,
    Texture as WGPUTexture, TextureAspect, TextureDescriptor, TextureDimension, TextureFormat,
    TextureSampleType, TextureUsages, TextureView, TextureViewDescriptor, TextureViewDimension,
    VertexState,
};

#[derive(Debug, Error)]
pub enum FromNativeResourceError {
    #[cfg(target_os = "windows")]
    #[error(transparent)]
    FromDxgiResourceError(#[from] FromDxgiResourceError),
}

pub enum HardwareTexture<'a> {
    #[cfg(target_os = "windows")]
    Dx11(&'a ID3D11Texture2D, &'a D3D11_TEXTURE2D_DESC, u32),
    #[cfg(any(target_os = "linux"))]
    Vulkan(&'a usize),
}

impl<'a> HardwareTexture<'a> {
    #[allow(unused)]
    pub(crate) fn texture(&self, device: &Device) -> Result<WGPUTexture, FromNativeResourceError> {
        Ok(match self {
            #[cfg(target_os = "windows")]
            HardwareTexture::Dx11(dx11, desc, _) => {
                create_texture_from_dx11_texture(device, dx11, desc)?
            }
            _ => unimplemented!("not supports native texture"),
        })
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
    /// Get the hardware texture, here does not deal with software texture, so
    /// if it is software texture directly return None.
    pub(crate) fn texture(
        &self,
        device: &Device,
    ) -> Result<Option<WGPUTexture>, FromNativeResourceError> {
        Ok(match self {
            TextureResource::Texture(texture) => texture.texture(device).ok(),
            _ => None,
        })
    }

    pub(crate) fn size(&self) -> Size {
        match self {
            TextureResource::Texture(texture) => match texture {
                HardwareTexture::Dx11(_, desc, _) => Size {
                    width: desc.Width,
                    height: desc.Height,
                },
            },
            TextureResource::Buffer(resource) => resource.size,
        }
    }
}

pub enum Texture<'a> {
    Rgba(TextureResource<'a>),
    Nv12(TextureResource<'a>),
    I420(SoftwareTexture<'a>),
}

impl<'a> Texture<'a> {
    pub(crate) fn texture(
        &self,
        device: &Device,
    ) -> Result<Option<WGPUTexture>, FromNativeResourceError> {
        Ok(match self {
            Texture::Rgba(texture) | Texture::Nv12(texture) => texture.texture(device)?,
            _ => None,
        })
    }

    pub(crate) fn size(&self) -> Size {
        match self {
            Texture::Rgba(texture) | Texture::Nv12(texture) => texture.size(),
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
                    // Bytes per “row” in an image.
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

/// RGBA stands for red green blue alpha. While it is sometimes described as a
/// color space, it is actually a three-channel RGB color model supplemented
/// with a fourth alpha channel. Alpha indicates how opaque each pixel is and
/// allows an image to be combined over others using alpha compositing, with
/// transparent areas and anti-aliasing of the edges of opaque regions. Each
/// pixel is a 4D vector.
///
/// The term does not define what RGB color space is being used. It also does
/// not state whether or not the colors are premultiplied by the alpha value,
/// and if they are it does not state what color space that premultiplication
/// was done in. This means more information than just "RGBA" is needed to
/// determine how to handle an image.
///
/// In some contexts the abbreviation "RGBA" means a specific memory layout
/// (called RGBA8888 below), with other terms such as "BGRA" used for
/// alternatives. In other contexts "RGBA" means any layout.
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
                width: size.width * 4,
                height: size.height,
            },
        )]
    }
}

/// YCbCr, Y′CbCr, or Y Pb/Cb Pr/Cr, also written as YCBCR or Y′CBCR, is a
/// family of color spaces used as a part of the color image pipeline in video
/// and digital photography systems. Y′ is the luma component and CB and CR are
/// the blue-difference and red-difference chroma components. Y′ (with prime) is
/// distinguished from Y, which is luminance, meaning that light intensity is
/// nonlinearly encoded based on gamma corrected RGB primaries.
///
/// Y′CbCr color spaces are defined by a mathematical coordinate transformation
/// from an associated RGB primaries and white point. If the underlying RGB
/// color space is absolute, the Y′CbCr color space is an absolute color space
/// as well; conversely, if the RGB space is ill-defined, so is Y′CbCr. The
/// transformation is defined in equations 32, 33 in ITU-T H.273. Nevertheless
/// that rule does not apply to P3-D65 primaries used by Netflix with
/// BT.2020-NCL matrix, so that means matrix was not derived from primaries, but
/// now Netflix allows BT.2020 primaries (since 2021).[1] The same happens with
/// JPEG: it has BT.601 matrix derived from System M primaries, yet the
/// primaries of most images are BT.709.
///
/// NV12 is possibly the most commonly-used 8-bit 4:2:0 format. It is the
/// default for Android camera preview.[19] The entire image in Y is written
/// out, followed by interleaved lines that go U0, V0, U1, V1, etc.
struct Nv12(WGPUTexture, WGPUTexture);

impl Nv12 {
    fn new(device: &Device, size: Size) -> Self {
        let mut textures = Self::create(device, size);
        Self(textures.next().unwrap(), textures.next().unwrap())
    }
}

impl Texture2DSample for Nv12 {
    fn create_texture_descriptor(size: Size) -> impl IntoIterator<Item = (Size, TextureFormat)> {
        [
            (size, TextureFormat::R8Unorm),
            (
                Size {
                    width: size.width / 2,
                    height: size.height / 2,
                },
                TextureFormat::Rg8Unorm,
            ),
        ]
    }

    fn views_descriptors<'a>(
        &'a self,
        texture: Option<&'a WGPUTexture>,
    ) -> impl IntoIterator<Item = (&'a WGPUTexture, TextureFormat, TextureAspect)> {
        // When you create a view directly for a texture, the external texture is a
        // single texture, and you need to create different planes of views on top of
        // the single texture.
        if let Some(texture) = texture {
            [
                (texture, TextureFormat::R8Unorm, TextureAspect::Plane0),
                (texture, TextureFormat::Rg8Unorm, TextureAspect::Plane1),
            ]
        } else {
            [
                (&self.0, TextureFormat::R8Unorm, TextureAspect::All),
                (&self.1, TextureFormat::Rg8Unorm, TextureAspect::All),
            ]
        }
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
            (buffers[0], &self.0, TextureAspect::All, size),
            (buffers[1], &self.1, TextureAspect::All, size),
        ]
    }
}

/// YCbCr, Y′CbCr, or Y Pb/Cb Pr/Cr, also written as YCBCR or Y′CBCR, is a
/// family of color spaces used as a part of the color image pipeline in video
/// and digital photography systems. Y′ is the luma component and CB and CR are
/// the blue-difference and red-difference chroma components. Y′ (with prime) is
/// distinguished from Y, which is luminance, meaning that light intensity is
/// nonlinearly encoded based on gamma corrected RGB primaries.
///
/// Y′CbCr color spaces are defined by a mathematical coordinate transformation
/// from an associated RGB primaries and white point. If the underlying RGB
/// color space is absolute, the Y′CbCr color space is an absolute color space
/// as well; conversely, if the RGB space is ill-defined, so is Y′CbCr. The
/// transformation is defined in equations 32, 33 in ITU-T H.273. Nevertheless
/// that rule does not apply to P3-D65 primaries used by Netflix with
/// BT.2020-NCL matrix, so that means matrix was not derived from primaries, but
/// now Netflix allows BT.2020 primaries (since 2021).[1] The same happens with
/// JPEG: it has BT.601 matrix derived from System M primaries, yet the
/// primaries of most images are BT.709.
struct I420(WGPUTexture, WGPUTexture, WGPUTexture);

impl I420 {
    fn new(device: &Device, size: Size) -> Self {
        let mut textures = Self::create(device, size);
        Self(
            textures.next().unwrap(),
            textures.next().unwrap(),
            textures.next().unwrap(),
        )
    }
}

impl Texture2DSample for I420 {
    fn create_texture_descriptor(size: Size) -> impl IntoIterator<Item = (Size, TextureFormat)> {
        [
            (size, TextureFormat::R8Unorm),
            (
                Size {
                    width: size.width / 2,
                    height: size.height / 2,
                },
                TextureFormat::R8Unorm,
            ),
            (
                Size {
                    width: size.width / 2,
                    height: size.height / 2,
                },
                TextureFormat::R8Unorm,
            ),
        ]
    }

    fn views_descriptors<'a>(
        &'a self,
        _: Option<&'a WGPUTexture>,
    ) -> impl IntoIterator<Item = (&'a WGPUTexture, TextureFormat, TextureAspect)> {
        [
            (&self.0, TextureFormat::R8Unorm, TextureAspect::All),
            (&self.1, TextureFormat::R8Unorm, TextureAspect::All),
            (&self.2, TextureFormat::R8Unorm, TextureAspect::All),
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
            (buffers[0], &self.0, TextureAspect::All, size),
            (
                buffers[1],
                &self.1,
                TextureAspect::All,
                Size {
                    width: size.width / 2,
                    height: size.height / 2,
                },
            ),
            (
                buffers[2],
                &self.2,
                TextureAspect::All,
                Size {
                    width: size.width / 2,
                    height: size.height / 2,
                },
            ),
        ]
    }
}

enum Texture2DSourceSample {
    Rgba(Rgba),
    Nv12(Nv12),
    I420(I420),
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
            let sample = match &texture {
                Texture::Rgba(texture) => {
                    Texture2DSourceSample::Rgba(Rgba::new(&self.device, texture.size()))
                }
                Texture::Nv12(texture) => {
                    Texture2DSourceSample::Nv12(Nv12::new(&self.device, texture.size()))
                }
                Texture::I420(texture) => {
                    Texture2DSourceSample::I420(I420::new(&self.device, texture.size))
                }
            };

            let bind_group_layout = match &sample {
                Texture2DSourceSample::Rgba(texture) => texture.bind_group_layout(&self.device),
                Texture2DSourceSample::Nv12(texture) => texture.bind_group_layout(&self.device),
                Texture2DSourceSample::I420(texture) => texture.bind_group_layout(&self.device),
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
                                // Because the output surface is RGBA, RGBA is a generic texture
                                // format.
                                Texture2DSourceSample::Rgba(_) => {
                                    include_wgsl!("./shaders/fragment/any.wgsl")
                                }
                                Texture2DSourceSample::Nv12(_) => {
                                    include_wgsl!("./shaders/fragment/nv12.wgsl")
                                }
                                Texture2DSourceSample::I420(_) => {
                                    include_wgsl!("./shaders/fragment/i420.wgsl")
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

        // Only software textures need to be updated to the sample via update.
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
                        Texture2DSourceSample::I420(sample) => {
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
