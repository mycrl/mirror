use rubato::{
    FastFixedIn, PolynomialDegree, ResampleResult, Resampler, ResamplerConstructionError,
};

/// Audio resampler, quickly resample input to a single channel count and
/// different sampling rates.
///
/// Note that due to the fast sampling, the quality may be reduced.
pub struct AudioResampler {
    sampler: Option<FastFixedIn<f32>>,
    input_buffer: Vec<f32>,
    output_buffer: Vec<f32>,
    samples: Vec<i16>,
}

impl AudioResampler {
    pub fn new(input: f64, output: f64, frames: usize) -> Result<Self, ResamplerConstructionError> {
        Ok(Self {
            samples: Vec::with_capacity(frames),
            input_buffer: Vec::with_capacity(48000),
            output_buffer: vec![0.0; 48000],
            sampler: if input != output {
                Some(FastFixedIn::new(
                    output / input,
                    2.0,
                    PolynomialDegree::Linear,
                    frames,
                    1,
                )?)
            } else {
                None
            },
        })
    }

    pub fn resample<'a>(
        &'a mut self,
        buffer: &'a [i16],
        channels: usize,
    ) -> ResampleResult<&'a [i16]> {
        if channels == 1 && self.sampler.is_none() {
            Ok(buffer)
        } else {
            self.samples.clear();
            self.input_buffer.clear();

            for item in buffer.iter().step_by(channels) {
                if self.sampler.is_none() {
                    self.samples.push(*item);
                } else {
                    // need resample
                    self.input_buffer.push(*item as f32);
                }
            }

            if let Some(sampler) = &mut self.sampler {
                let (_, size) = sampler.process_into_buffer(
                    &[&self.input_buffer[..]],
                    &mut [&mut self.output_buffer],
                    None,
                )?;

                for item in &self.output_buffer[..size] {
                    self.samples.push(*item as i16);
                }
            }

            Ok(&self.samples[..])
        }
    }
}

#[cfg(target_os = "windows")]
pub mod win32 {
    use std::mem::ManuallyDrop;

    use common::{
        win32::{
            windows::{
                core::{Error, Interface},
                Win32::{
                    Foundation::RECT,
                    Graphics::{
                        Direct3D11::{
                            ID3D11Device, ID3D11DeviceContext, ID3D11Texture2D, ID3D11VideoContext,
                            ID3D11VideoDevice, ID3D11VideoProcessor,
                            ID3D11VideoProcessorEnumerator, ID3D11VideoProcessorInputView,
                            ID3D11VideoProcessorOutputView, D3D11_BIND_RENDER_TARGET,
                            D3D11_CPU_ACCESS_READ, D3D11_MAPPED_SUBRESOURCE, D3D11_MAP_READ,
                            D3D11_RESOURCE_MISC_SHARED, D3D11_TEXTURE2D_DESC, D3D11_USAGE_DEFAULT,
                            D3D11_USAGE_STAGING, D3D11_VIDEO_FRAME_FORMAT_PROGRESSIVE,
                            D3D11_VIDEO_PROCESSOR_COLOR_SPACE, D3D11_VIDEO_PROCESSOR_CONTENT_DESC,
                            D3D11_VIDEO_PROCESSOR_INPUT_VIEW_DESC,
                            D3D11_VIDEO_PROCESSOR_OUTPUT_VIEW_DESC, D3D11_VIDEO_PROCESSOR_STREAM,
                            D3D11_VIDEO_USAGE_PLAYBACK_NORMAL, D3D11_VPIV_DIMENSION_TEXTURE2D,
                            D3D11_VPOV_DIMENSION_TEXTURE2D,
                        },
                        Dxgi::Common::DXGI_FORMAT,
                    },
                },
            },
            Direct3DDevice,
        },
        Size,
    };

    pub enum Resource {
        Default(DXGI_FORMAT, Size),
        Texture(ID3D11Texture2D),
    }

    pub struct VideoResamplerDescriptor {
        pub direct3d: Direct3DDevice,
        pub input: Resource,
        pub output: Resource,
    }

    /// Used to convert video frames using hardware accelerators, including
    /// color space conversion and scaling. Note that the output is fixed to
    /// NV12, but the input is optional and is RGBA by default. However, if
    /// you use the `process` method, you can let the external texture
    /// decide what format to use, because this method does not copy the
    /// texture.
    #[allow(unused)]
    pub struct VideoResampler {
        d3d_device: ID3D11Device,
        d3d_context: ID3D11DeviceContext,
        video_device: ID3D11VideoDevice,
        video_context: ID3D11VideoContext,
        input_texture: ID3D11Texture2D,
        output_texture: ID3D11Texture2D,
        video_enumerator: ID3D11VideoProcessorEnumerator,
        video_processor: ID3D11VideoProcessor,
        input_view: ID3D11VideoProcessorInputView,
        output_view: ID3D11VideoProcessorOutputView,
    }

    unsafe impl Send for VideoResampler {}
    unsafe impl Sync for VideoResampler {}

    impl VideoResampler {
        /// Create `VideoResampler`, the default_device parameter is used to
        /// directly use the device when it has been created externally, so
        /// there is no need to copy across devices, which improves
        /// processing performance.
        pub fn new(options: VideoResamplerDescriptor) -> Result<Self, Error> {
            let (d3d_device, d3d_context) = (options.direct3d.device, options.direct3d.context);
            let video_device = d3d_device.cast::<ID3D11VideoDevice>()?;
            let video_context = d3d_context.cast::<ID3D11VideoContext>()?;

            let input_texture = match options.input {
                Resource::Texture(texture) => texture,
                Resource::Default(format, size) => unsafe {
                    let mut desc = D3D11_TEXTURE2D_DESC::default();
                    desc.Width = size.width;
                    desc.Height = size.height;
                    desc.MipLevels = 1;
                    desc.ArraySize = 1;
                    desc.Format = format.into();
                    desc.SampleDesc.Count = 1;
                    desc.SampleDesc.Quality = 0;
                    desc.Usage = D3D11_USAGE_DEFAULT;
                    desc.BindFlags = D3D11_BIND_RENDER_TARGET.0 as u32;
                    desc.CPUAccessFlags = 0;
                    desc.MiscFlags = 0;

                    let mut texture = None;
                    d3d_device.CreateTexture2D(&desc, None, Some(&mut texture))?;
                    texture.unwrap()
                },
            };

            let output_texture = match options.output {
                Resource::Texture(texture) => texture,
                Resource::Default(format, size) => unsafe {
                    let mut desc = D3D11_TEXTURE2D_DESC::default();
                    desc.Width = size.width;
                    desc.Height = size.height;
                    desc.MipLevels = 1;
                    desc.ArraySize = 1;
                    desc.Format = format.into();
                    desc.SampleDesc.Count = 1;
                    desc.SampleDesc.Quality = 0;
                    desc.Usage = D3D11_USAGE_DEFAULT;
                    desc.BindFlags = D3D11_BIND_RENDER_TARGET.0 as u32;
                    desc.CPUAccessFlags = 0;
                    desc.MiscFlags = D3D11_RESOURCE_MISC_SHARED.0 as u32;

                    let mut texture = None;
                    d3d_device.CreateTexture2D(&desc, None, Some(&mut texture))?;
                    texture.unwrap()
                },
            };

            let mut input_desc = D3D11_TEXTURE2D_DESC::default();
            unsafe {
                input_texture.GetDesc(&mut input_desc);
            }

            let mut output_desc = D3D11_TEXTURE2D_DESC::default();
            unsafe {
                output_texture.GetDesc(&mut output_desc);
            }

            let (video_enumerator, video_processor) = unsafe {
                let mut desc = D3D11_VIDEO_PROCESSOR_CONTENT_DESC::default();
                desc.InputFrameFormat = D3D11_VIDEO_FRAME_FORMAT_PROGRESSIVE;
                desc.InputWidth = input_desc.Width;
                desc.InputHeight = input_desc.Height;
                desc.OutputWidth = output_desc.Width;
                desc.OutputHeight = output_desc.Height;
                desc.Usage = D3D11_VIDEO_USAGE_PLAYBACK_NORMAL;

                let enumerator = video_device.CreateVideoProcessorEnumerator(&desc)?;
                let processor = video_device.CreateVideoProcessor(&enumerator, 0)?;
                (enumerator, processor)
            };

            let input_view = unsafe {
                let mut desc = D3D11_VIDEO_PROCESSOR_INPUT_VIEW_DESC::default();
                desc.FourCC = 0;
                desc.ViewDimension = D3D11_VPIV_DIMENSION_TEXTURE2D;
                desc.Anonymous.Texture2D.MipSlice = 0;

                let mut view = None;
                video_device.CreateVideoProcessorInputView(
                    &input_texture,
                    &video_enumerator,
                    &desc,
                    Some(&mut view),
                )?;

                view.unwrap()
            };

            let output_view = unsafe {
                let mut desc = D3D11_VIDEO_PROCESSOR_OUTPUT_VIEW_DESC::default();
                desc.ViewDimension = D3D11_VPOV_DIMENSION_TEXTURE2D;

                let mut view = None;
                video_device.CreateVideoProcessorOutputView(
                    &output_texture,
                    &video_enumerator,
                    &desc,
                    Some(&mut view),
                )?;

                view.unwrap()
            };

            unsafe {
                video_context.VideoProcessorSetStreamSourceRect(
                    &video_processor,
                    0,
                    true,
                    Some(&RECT {
                        left: 0,
                        top: 0,
                        right: input_desc.Width as i32,
                        bottom: input_desc.Height as i32,
                    }),
                );
            }

            unsafe {
                video_context.VideoProcessorSetStreamDestRect(
                    &video_processor,
                    0,
                    true,
                    Some(&RECT {
                        left: 0,
                        top: 0,
                        right: output_desc.Width as i32,
                        bottom: output_desc.Height as i32,
                    }),
                );
            }

            unsafe {
                let color_space = D3D11_VIDEO_PROCESSOR_COLOR_SPACE::default();
                video_context.VideoProcessorSetStreamColorSpace(&video_processor, 0, &color_space);
            }

            Ok(Self {
                d3d_device,
                d3d_context,
                video_device,
                video_context,
                video_enumerator,
                video_processor,
                input_texture,
                output_texture,
                input_view,
                output_view,
            })
        }

        /// To update the internal texture, simply copy it to the internal
        /// texture.
        pub fn update_input(&mut self, texture: &ID3D11Texture2D) {
            unsafe {
                self.d3d_context.CopyResource(&self.input_texture, texture);
            }
        }

        /// Perform the conversion. This method will copy the texture array to
        /// the internal texture, so there are restrictions on the
        /// format of the incoming texture. Because the internal one is
        /// fixed to RGBA, the external texture can only be RGBA.
        pub fn update_input_from_buffer(
            &mut self,
            buf: *const u8,
            stride: u32,
        ) -> Result<(), Error> {
            unsafe {
                self.d3d_context.UpdateSubresource(
                    &self.input_texture,
                    0,
                    None,
                    buf as *const _,
                    stride,
                    0,
                );
            }

            Ok(())
        }

        /// Perform the conversion. This method will not copy the passed
        /// texture, but will use the texture directly, which can save a
        /// copy step and improve performance.
        pub fn create_input_view(
            &mut self,
            texture: &ID3D11Texture2D,
            index: u32,
        ) -> Result<ID3D11VideoProcessorInputView, Error> {
            let input_view = unsafe {
                let mut desc = D3D11_VIDEO_PROCESSOR_INPUT_VIEW_DESC::default();
                desc.FourCC = 0;
                desc.ViewDimension = D3D11_VPIV_DIMENSION_TEXTURE2D;
                desc.Anonymous.Texture2D.MipSlice = 0;
                desc.Anonymous.Texture2D.ArraySlice = index;

                let mut view = None;
                self.video_device.CreateVideoProcessorInputView(
                    texture,
                    &self.video_enumerator,
                    &desc,
                    Some(&mut view),
                )?;

                view.unwrap()
            };

            Ok(input_view)
        }

        pub fn get_output(&self) -> &ID3D11Texture2D {
            &self.output_texture
        }

        pub fn get_output_buffer(&mut self) -> Result<TextureBuffer, Error> {
            Ok(TextureBuffer::new(
                &self.d3d_device,
                &self.d3d_context,
                &self.output_texture,
            )?)
        }

        pub fn process(
            &mut self,
            input_view: Option<ID3D11VideoProcessorInputView>,
        ) -> Result<(), Error> {
            unsafe {
                let mut streams = [D3D11_VIDEO_PROCESSOR_STREAM::default()];
                streams[0].Enable = true.into();
                streams[0].OutputIndex = 0;
                streams[0].InputFrameOrField = 0;
                streams[0].pInputSurface =
                    ManuallyDrop::new(Some(input_view.unwrap_or_else(|| self.input_view.clone())));

                self.video_context.VideoProcessorBlt(
                    &self.video_processor,
                    &self.output_view,
                    0,
                    &streams,
                )?;

                ManuallyDrop::drop(&mut streams[0].pInputSurface);
            }

            Ok(())
        }
    }

    pub struct TextureBuffer<'a> {
        d3d_context: &'a ID3D11DeviceContext,
        texture: ID3D11Texture2D,
        resource: D3D11_MAPPED_SUBRESOURCE,
    }

    unsafe impl Send for TextureBuffer<'_> {}
    unsafe impl Sync for TextureBuffer<'_> {}

    impl<'a> TextureBuffer<'a> {
        pub fn new(
            d3d_device: &ID3D11Device,
            d3d_context: &'a ID3D11DeviceContext,
            source_texture: &ID3D11Texture2D,
        ) -> Result<Self, Error> {
            let texture = unsafe {
                let mut desc = D3D11_TEXTURE2D_DESC::default();
                source_texture.GetDesc(&mut desc);

                desc.CPUAccessFlags = D3D11_CPU_ACCESS_READ.0 as u32;
                desc.Usage = D3D11_USAGE_STAGING;
                desc.BindFlags = 0;
                desc.MiscFlags = 0;

                let mut texture = None;
                d3d_device.CreateTexture2D(&desc, None, Some(&mut texture))?;
                texture.unwrap()
            };

            unsafe {
                d3d_context.CopyResource(&texture, source_texture);
            }

            let mut resource = D3D11_MAPPED_SUBRESOURCE::default();
            unsafe {
                d3d_context.Map(&texture, 0, D3D11_MAP_READ, 0, Some(&mut resource))?;
            }

            Ok(Self {
                d3d_context,
                resource,
                texture,
            })
        }

        /// Represents a pointer to texture data. Internally, the texture is
        /// copied to the CPU first, and then the internal data is
        /// mapped.
        pub fn buffer(&self) -> *const u8 {
            self.resource.pData as *const _
        }

        /// The stride of the texture data
        pub fn stride(&self) -> usize {
            self.resource.RowPitch as usize
        }
    }

    impl Drop for TextureBuffer<'_> {
        fn drop(&mut self) {
            unsafe {
                self.d3d_context.Unmap(&self.texture, 0);
            }
        }
    }
}
