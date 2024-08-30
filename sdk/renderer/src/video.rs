use anyhow::{anyhow, Result};

#[derive(Debug, Clone, Copy)]
pub struct Size {
    pub width: u32,
    pub height: u32,
}

#[cfg(target_os = "windows")]
mod win32 {
    use frame::{Resource, VideoFrame, VideoSize, VideoTransform, VideoTransformOptions};
    use utils::win32::{d3d_texture_borrowed_raw, Direct3DDevice};
    use windows::Win32::{
        Foundation::HWND,
        Graphics::{
            Direct3D11::{ID3D11RenderTargetView, ID3D11Texture2D, D3D11_VIEWPORT},
            Dxgi::{
                Common::DXGI_FORMAT_R8G8B8A8_UNORM, CreateDXGIFactory, IDXGIFactory,
                IDXGISwapChain, DXGI_PRESENT, DXGI_SWAP_CHAIN_DESC,
                DXGI_USAGE_RENDER_TARGET_OUTPUT,
            },
        },
    };

    pub struct VideoRenderOptions {
        pub size: Size,
        pub window_handle: HWND,
        pub direct3d: Direct3DDevice,
    }

    pub struct VideoRender {
        direct3d: Direct3DDevice,
        swap_chain: IDXGISwapChain,
        render_target_view: ID3D11RenderTargetView,
        video_processor: Option<VideoTransform>,
    }

    unsafe impl Send for VideoRender {}
    unsafe impl Sync for VideoRender {}

    impl VideoRender {
        pub fn new(options: VideoRenderOptions) -> Result<Self> {
            log::info!("renderer: create video render, size={:?}", options.size);

            let direct3d = options.direct3d;
            let swap_chain = unsafe {
                let dxgi_factory = CreateDXGIFactory::<IDXGIFactory>()?;

                let mut desc = DXGI_SWAP_CHAIN_DESC::default();
                desc.BufferCount = 1;
                desc.BufferDesc.Width = options.size.width;
                desc.BufferDesc.Height = options.size.height;
                desc.BufferDesc.Format = DXGI_FORMAT_R8G8B8A8_UNORM;
                desc.BufferUsage = DXGI_USAGE_RENDER_TARGET_OUTPUT;
                desc.OutputWindow = options.window_handle;
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
                vp.Width = options.size.width as f32;
                vp.Height = options.size.height as f32;
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

        /// Draw this pixel buffer to the configured [`SurfaceTexture`].
        pub fn send(&mut self, frame: &VideoFrame) -> Result<()> {
            if frame.data[0].is_null() {
                return Err(anyhow!("frame texture is null"));
            }

            unsafe {
                self.direct3d
                    .context
                    .ClearRenderTargetView(&self.render_target_view, &[0.0, 0.0, 0.0, 1.0]);
            }

            if self.video_processor.is_none() {
                self.video_processor = Some(VideoTransform::new(VideoTransformOptions {
                    direct3d: self.direct3d.clone(),
                    input: Resource::Default(
                        frame.format,
                        VideoSize {
                            width: frame.width,
                            height: frame.height,
                        },
                    ),
                    output: Resource::Texture(unsafe {
                        self.swap_chain.GetBuffer::<ID3D11Texture2D>(0)?
                    }),
                })?);
            }

            if let Some(processor) = &mut self.video_processor {
                let view = if frame.hardware {
                    let texture = frame.data[0] as *mut _;
                    if let Some(texture) = d3d_texture_borrowed_raw(&texture) {
                        Some(processor.create_input_view(texture, frame.data[1] as u32)?)
                    } else {
                        None
                    }
                } else {
                    processor.update_input_from_buffer(
                        frame.data[0] as *const _,
                        frame.linesize[0] as u32,
                    )?;

                    None
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
