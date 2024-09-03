use std::{ffi::c_void, ptr::null};

#[derive(Debug, Clone, Copy)]
pub struct VideoSize {
    pub width: u32,
    pub height: u32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VideoFormat {
    RGBA,
    NV12,
}

/// YCbCr (NV12)
///
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
/// now Netflix allows BT.2020 primaries (since 2021). The same happens with
/// JPEG: it has BT.601 matrix derived from System M primaries, yet the
/// primaries of most images are BT.709.
#[repr(C)]
#[derive(Debug)]
pub struct VideoFrame {
    pub format: VideoFormat,
    pub hardware: bool,
    pub width: u32,
    pub height: u32,
    pub data: [*const c_void; 2],
    pub linesize: [usize; 2],
}

unsafe impl Sync for VideoFrame {}
unsafe impl Send for VideoFrame {}

impl Default for VideoFrame {
    fn default() -> Self {
        Self {
            width: 0,
            height: 0,
            hardware: false,
            linesize: [0, 0],
            data: [null(), null()],
            format: VideoFormat::RGBA,
        }
    }
}

#[cfg(target_os = "windows")]
pub mod win32 {
    use super::{VideoFormat, VideoSize};

    use std::mem::ManuallyDrop;

    use utils::win32::Direct3DDevice;
    use windows::{
        core::{Interface, Result},
        Win32::{
            Foundation::{HANDLE, RECT},
            Graphics::{
                Direct3D11::{
                    ID3D11Device, ID3D11DeviceContext, ID3D11Texture2D, ID3D11VideoContext,
                    ID3D11VideoDevice, ID3D11VideoProcessor, ID3D11VideoProcessorEnumerator,
                    ID3D11VideoProcessorInputView, ID3D11VideoProcessorOutputView,
                    D3D11_BIND_RENDER_TARGET, D3D11_CPU_ACCESS_READ, D3D11_MAPPED_SUBRESOURCE,
                    D3D11_MAP_READ, D3D11_TEXTURE2D_DESC, D3D11_USAGE_DEFAULT, D3D11_USAGE_STAGING,
                    D3D11_VIDEO_FRAME_FORMAT_PROGRESSIVE, D3D11_VIDEO_PROCESSOR_COLOR_SPACE,
                    D3D11_VIDEO_PROCESSOR_CONTENT_DESC, D3D11_VIDEO_PROCESSOR_INPUT_VIEW_DESC,
                    D3D11_VIDEO_PROCESSOR_OUTPUT_VIEW_DESC, D3D11_VIDEO_PROCESSOR_STREAM,
                    D3D11_VIDEO_USAGE_PLAYBACK_NORMAL, D3D11_VPIV_DIMENSION_TEXTURE2D,
                    D3D11_VPOV_DIMENSION_TEXTURE2D,
                },
                Dxgi::Common::{DXGI_FORMAT, DXGI_FORMAT_NV12, DXGI_FORMAT_R8G8B8A8_UNORM},
            },
        },
    };

    impl Into<DXGI_FORMAT> for super::VideoFormat {
        fn into(self) -> DXGI_FORMAT {
            match self {
                Self::RGBA => DXGI_FORMAT_R8G8B8A8_UNORM,
                Self::NV12 => DXGI_FORMAT_NV12,
            }
        }
    }

    pub enum Resource {
        Default(VideoFormat, VideoSize),
        Texture(ID3D11Texture2D),
    }

    pub struct VideoTransformOptions {
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
    pub struct VideoTransform {
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

    unsafe impl Send for VideoTransform {}
    unsafe impl Sync for VideoTransform {}

    impl VideoTransform {
        /// Create `VideoTransform`, the default_device parameter is used to
        /// directly use the device when it has been created externally, so
        /// there is no need to copy across devices, which improves
        /// processing performance.
        pub fn new(options: VideoTransformOptions) -> Result<Self> {
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
                    desc.MiscFlags = 0;

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

        /// open shared texture.
        pub fn open_shared_texture(&mut self, handle: HANDLE) -> Result<ID3D11Texture2D> {
            Ok(unsafe {
                let mut texture: Option<ID3D11Texture2D> = None;
                self.d3d_device.OpenSharedResource(handle, &mut texture)?;
                texture.unwrap()
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
        pub fn update_input_from_buffer(&mut self, buf: *const u8, stride: u32) -> Result<()> {
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
        ) -> Result<ID3D11VideoProcessorInputView> {
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

        pub fn get_output_buffer(&mut self) -> Result<TextureBuffer> {
            Ok(TextureBuffer::new(
                &self.d3d_device,
                &self.d3d_context,
                &self.output_texture,
            )?)
        }

        pub fn process(&mut self, input_view: Option<ID3D11VideoProcessorInputView>) -> Result<()> {
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
        ) -> Result<Self> {
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

#[cfg(target_os = "linux")]
pub mod unix {
    use super::{VideoSize, VideoFormat};

    pub struct VideoTransform {
        input: VideoSize,
        output: VideoSize,
        source: Vec<u8>,
        scaled: Vec<u8>,
    }

    impl VideoTransform {
        pub fn new(input: VideoSize, output: VideoSize) -> Self {
            Self {
                source: vec![0u8; (input.width as f64 * input.height as f64 * 1.5) as usize],
                scaled: vec![0u8; (output.width as f64 * output.height as f64 * 1.5) as usize],
                input,
                output,
            }
        }

        pub fn process(&mut self, texture: &[u8], format: VideoFormat) -> &[u8] {
            match format {
                VideoFormat::RGBA => unsafe {
                    libyuv::argb_to_nv12(
                        texture.as_ptr(),
                        self.input.width as i32 * 4,
                        self.source.as_mut_ptr(),
                        self.input.width as i32,
                        self.source
                            .as_mut_ptr()
                            .add(self.input.width as usize * self.input.height as usize),
                        self.input.width as i32,
                        self.input.width as i32,
                        self.input.height as i32,
                    );
                },
                VideoFormat::NV12 => unsafe {
                    libyuv::yuy2_to_nv12(
                        texture.as_ptr(),
                        self.input.width as i32,
                        self.source.as_mut_ptr(),
                        self.input.width as i32,
                        self.source
                            .as_mut_ptr()
                            .add(self.input.width as usize * self.input.height as usize),
                        self.input.width as i32,
                        self.input.width as i32,
                        self.input.height as i32,
                    );
                },
            }

            unsafe {
                libyuv::nv12_scale(
                    self.source.as_ptr(),
                    self.input.width as i32,
                    self.source
                        .as_ptr()
                        .add(self.input.width as usize * self.input.height as usize),
                    self.input.width as i32,
                    self.input.width as i32,
                    self.input.height as i32,
                    self.scaled.as_mut_ptr(),
                    self.output.width as i32,
                    self.scaled
                        .as_mut_ptr()
                        .add(self.output.width as usize * self.output.height as usize),
                    self.output.width as i32,
                    self.output.width as i32,
                    self.output.height as i32,
                    libyuv::FilterMode::FilterLinear,
                );
            }

            &self.scaled
        }
    }
}
