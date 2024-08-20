use std::ptr::null;

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
    pub width: u32,
    pub height: u32,
    pub data: [*const u8; 2],
    pub linesize: [usize; 2],
}

unsafe impl Sync for VideoFrame {}
unsafe impl Send for VideoFrame {}

impl Default for VideoFrame {
    fn default() -> Self {
        Self {
            width: 0,
            height: 0,
            linesize: [0, 0],
            data: [null(), null()],
        }
    }
}

#[derive(Debug)]
pub struct VideoSize {
    pub width: u32,
    pub height: u32,
}

#[cfg(target_os = "windows")]
pub mod win32 {
    use super::VideoSize;

    use std::mem::ManuallyDrop;

    use windows::{
        core::{Interface, Result},
        Win32::{
            Foundation::RECT,
            Graphics::{
                Direct3D::{
                    D3D_DRIVER_TYPE_HARDWARE, D3D_FEATURE_LEVEL, D3D_FEATURE_LEVEL_11_0,
                    D3D_FEATURE_LEVEL_11_1,
                },
                Direct3D11::{
                    D3D11CreateDevice, ID3D11Device, ID3D11DeviceContext, ID3D11Texture2D,
                    ID3D11VideoContext, ID3D11VideoDevice, ID3D11VideoProcessor,
                    ID3D11VideoProcessorEnumerator, ID3D11VideoProcessorInputView,
                    ID3D11VideoProcessorOutputView, D3D11_BIND_RENDER_TARGET, D3D11_BOX,
                    D3D11_CPU_ACCESS_READ, D3D11_CREATE_DEVICE_FLAG, D3D11_MAPPED_SUBRESOURCE,
                    D3D11_MAP_READ, D3D11_SDK_VERSION, D3D11_TEXTURE2D_DESC, D3D11_USAGE_DEFAULT,
                    D3D11_USAGE_STAGING, D3D11_VIDEO_FRAME_FORMAT_PROGRESSIVE,
                    D3D11_VIDEO_PROCESSOR_COLOR_SPACE, D3D11_VIDEO_PROCESSOR_CONTENT_DESC,
                    D3D11_VIDEO_PROCESSOR_INPUT_VIEW_DESC, D3D11_VIDEO_PROCESSOR_OUTPUT_VIEW_DESC,
                    D3D11_VIDEO_PROCESSOR_STREAM, D3D11_VIDEO_USAGE_PLAYBACK_NORMAL,
                    D3D11_VPIV_DIMENSION_TEXTURE2D, D3D11_VPOV_DIMENSION_TEXTURE2D,
                },
                Dxgi::Common::{DXGI_FORMAT_NV12, DXGI_FORMAT_R8G8B8A8_UNORM},
            },
        },
    };

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
        // Only use d3d11 implementation
        const FEATURE_LEVELS: [D3D_FEATURE_LEVEL; 2] =
            [D3D_FEATURE_LEVEL_11_1, D3D_FEATURE_LEVEL_11_0];

        /// Create `VideoTransform`, the default_device parameter is used to
        /// directly use the device when it has been created externally, so
        /// there is no need to copy across devices, which improves
        /// processing performance.
        pub fn new(
            input: VideoSize,
            output: VideoSize,
            default_device: Option<(ID3D11Device, ID3D11DeviceContext)>,
        ) -> Result<Self> {
            let (d3d_device, d3d_context) = if let Some(default_device) = default_device {
                default_device
            } else {
                unsafe {
                    let (mut d3d_device, mut d3d_context, mut feature_level) =
                        (None, None, D3D_FEATURE_LEVEL::default());

                    D3D11CreateDevice(
                        None,
                        D3D_DRIVER_TYPE_HARDWARE,
                        None,
                        D3D11_CREATE_DEVICE_FLAG(0),
                        Some(&Self::FEATURE_LEVELS),
                        D3D11_SDK_VERSION,
                        Some(&mut d3d_device),
                        Some(&mut feature_level),
                        Some(&mut d3d_context),
                    )?;

                    (d3d_device.unwrap(), d3d_context.unwrap())
                }
            };

            let video_device = d3d_device.cast::<ID3D11VideoDevice>()?;
            let video_context = d3d_context.cast::<ID3D11VideoContext>()?;

            let input_texture = unsafe {
                let mut desc = D3D11_TEXTURE2D_DESC::default();
                desc.Width = input.width;
                desc.Height = input.height;
                desc.MipLevels = 1;
                desc.ArraySize = 1;
                desc.Format = DXGI_FORMAT_R8G8B8A8_UNORM;
                desc.SampleDesc.Count = 1;
                desc.SampleDesc.Quality = 0;
                desc.Usage = D3D11_USAGE_DEFAULT;
                desc.BindFlags = D3D11_BIND_RENDER_TARGET.0 as u32;
                desc.CPUAccessFlags = 0;
                desc.MiscFlags = 0;

                let mut texture = None;
                d3d_device.CreateTexture2D(&desc, None, Some(&mut texture))?;
                texture.unwrap()
            };

            let output_texture = unsafe {
                let mut desc = D3D11_TEXTURE2D_DESC::default();
                desc.Width = output.width;
                desc.Height = output.height;
                desc.MipLevels = 1;
                desc.ArraySize = 1;
                desc.Format = DXGI_FORMAT_NV12;
                desc.SampleDesc.Count = 1;
                desc.SampleDesc.Quality = 0;
                desc.Usage = D3D11_USAGE_DEFAULT;
                desc.BindFlags = D3D11_BIND_RENDER_TARGET.0 as u32;
                desc.CPUAccessFlags = 0;
                desc.MiscFlags = 0;

                let mut texture = None;
                d3d_device.CreateTexture2D(&desc, None, Some(&mut texture))?;
                texture.unwrap()
            };

            let (video_enumerator, video_processor) = unsafe {
                let mut desc = D3D11_VIDEO_PROCESSOR_CONTENT_DESC::default();
                desc.InputFrameFormat = D3D11_VIDEO_FRAME_FORMAT_PROGRESSIVE;
                desc.InputWidth = input.width;
                desc.InputHeight = input.height;
                desc.OutputWidth = output.width;
                desc.OutputHeight = output.height;
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
                        right: input.width as i32,
                        bottom: input.height as i32,
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
                        right: output.width as i32,
                        bottom: output.height as i32,
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
        pub fn update(&mut self, texture: &ID3D11Texture2D) {
            unsafe {
                self.d3d_context.CopyResource(&self.input_texture, texture);
            }
        }

        /// Directly convert the current internal texture and get the converted
        /// result.
        pub fn get_output(&mut self) -> Result<Texture> {
            self.blt(self.input_view.clone())
        }

        /// Perform the conversion. This method will not copy the passed
        /// texture, but will use the texture directly, which can save a
        /// copy step and improve performance.
        pub fn process(&mut self, texture: &ID3D11Texture2D) -> Result<Texture> {
            let input_view = unsafe {
                let mut desc = D3D11_VIDEO_PROCESSOR_INPUT_VIEW_DESC::default();
                desc.FourCC = 0;
                desc.ViewDimension = D3D11_VPIV_DIMENSION_TEXTURE2D;
                desc.Anonymous.Texture2D.MipSlice = 0;

                let mut view = None;
                self.video_device.CreateVideoProcessorInputView(
                    texture,
                    &self.video_enumerator,
                    &desc,
                    Some(&mut view),
                )?;

                view.unwrap()
            };

            self.blt(input_view)
        }

        /// Perform the conversion. This method will copy the passed in texture,
        /// so there are restrictions on the format of the passed in
        /// texture. Because the internal texture is fixed to RGBA, the
        /// external texture can only be RGBA.
        pub fn process_with_copy(&mut self, texture: &ID3D11Texture2D) -> Result<Texture> {
            self.update(texture);
            self.blt(self.input_view.clone())
        }

        /// Perform the conversion. This method will copy the texture array to
        /// the internal texture, so there are restrictions on the
        /// format of the incoming texture. Because the internal one is
        /// fixed to RGBA, the external texture can only be RGBA.
        pub fn process_buffer(&mut self, buf: &[u8], size: VideoSize) -> Result<Texture> {
            unsafe {
                let mut dbox = D3D11_BOX::default();
                dbox.left = 0;
                dbox.top = 0;
                dbox.front = 0;
                dbox.back = 1;
                dbox.right = size.width;
                dbox.bottom = size.height;
                self.d3d_context.UpdateSubresource(
                    &self.input_texture,
                    0,
                    Some(&dbox),
                    buf.as_ptr() as *const _,
                    size.width * 4,
                    size.width * size.height,
                );
            }

            self.blt(self.input_view.clone())
        }

        fn blt(&mut self, input_view: ID3D11VideoProcessorInputView) -> Result<Texture> {
            unsafe {
                let mut streams = [D3D11_VIDEO_PROCESSOR_STREAM::default()];
                streams[0].Enable = true.into();
                streams[0].OutputIndex = 0;
                streams[0].InputFrameOrField = 0;
                streams[0].pInputSurface = ManuallyDrop::new(Some(input_view));
                self.video_context.VideoProcessorBlt(
                    &self.video_processor,
                    &self.output_view,
                    0,
                    &streams,
                )?;

                ManuallyDrop::drop(&mut streams[0].pInputSurface);
            }

            Ok(Texture::new(
                &self.d3d_device,
                &self.d3d_context,
                &self.output_texture,
            )?)
        }
    }

    pub struct Texture<'a> {
        d3d_context: &'a ID3D11DeviceContext,
        texture: ID3D11Texture2D,
        resource: D3D11_MAPPED_SUBRESOURCE,
    }

    unsafe impl Send for Texture<'_> {}
    unsafe impl Sync for Texture<'_> {}

    impl<'a> Texture<'a> {
        fn new(
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

    impl Drop for Texture<'_> {
        fn drop(&mut self) {
            unsafe {
                self.d3d_context.Unmap(&self.texture, 0);
            }
        }
    }
}

#[cfg(target_os = "linux")]
pub mod unix {
    use super::VideoSize;

    pub enum VideoFormat {
        ARGB,
        YUY2,
    }

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
                VideoFormat::ARGB => unsafe {
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
                VideoFormat::YUY2 => unsafe {
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
