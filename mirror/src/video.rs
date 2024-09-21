#[cfg(feature = "wgpu")]
pub mod general {
    use crate::Window;

    use anyhow::{anyhow, Result};
    use frame::{VideoFormat, VideoFrame};
    use graphics::{HardwareTexture, Renderer, SoftwareTexture, Texture, TextureResource};
    use utils::{
        win32::{
            d3d_texture_borrowed_raw,
            windows::Win32::Graphics::Direct3D11::D3D11_RESOURCE_MISC_SHARED, Direct3DDevice,
            EasyTexture,
        },
        Size,
    };

    pub struct VideoPlayer {
        direct3d: Option<Direct3DDevice>,
        renderer: Renderer<'static>,
    }

    impl VideoPlayer {
        pub fn new(window: Window) -> Result<Self> {
            let size = window.size()?;
            Ok(Self {
                renderer: Renderer::new(window, size)?,
                direct3d: crate::DIRECT_3D_DEVICE.read().unwrap().clone(),
            })
        }

        pub fn send(&mut self, frame: &VideoFrame) -> Result<()> {
            if frame.hardware {
                let mut dx_tex = d3d_texture_borrowed_raw(&(frame.data[0] as *mut _))
                    .cloned()
                    .ok_or_else(|| anyhow!("not found a texture"))?;

                let mut desc = dx_tex.desc();

                // Check if the texture supports creating shared resources, if not create a new
                // shared texture and copy it to the shared texture.
                if let Some(direct3d) = &self.direct3d {
                    if desc.MiscFlags & D3D11_RESOURCE_MISC_SHARED.0 as u32 == 0 {
                        desc.MiscFlags = D3D11_RESOURCE_MISC_SHARED.0 as u32;
                        desc.CPUAccessFlags = 0;
                        desc.BindFlags = 0;
                        desc.ArraySize = 1;
                        desc.MipLevels = 1;

                        dx_tex = unsafe {
                            let mut tex = None;
                            direct3d
                                .device
                                .CreateTexture2D(&desc, None, Some(&mut tex))?;
                            let tex = tex.unwrap();

                            direct3d.context.CopyResource(&tex, &dx_tex);
                            tex
                        };
                    }
                }

                let texture = TextureResource::Texture(HardwareTexture::Dx11(&dx_tex, &desc));
                self.renderer.submit(match frame.format {
                    VideoFormat::RGBA => Texture::Rgba(texture),
                    VideoFormat::NV12 => Texture::Nv12(texture),
                })?;
            } else {
                let buffers = match frame.format {
                    VideoFormat::RGBA => [
                        unsafe {
                            std::slice::from_raw_parts(
                                frame.data[0] as *const _,
                                frame.linesize[0] as usize * frame.height as usize * 4,
                            )
                        },
                        &[],
                        &[],
                    ],
                    VideoFormat::NV12 => [
                        unsafe {
                            std::slice::from_raw_parts(
                                frame.data[0] as *const _,
                                frame.linesize[0] as usize * frame.height as usize,
                            )
                        },
                        unsafe {
                            std::slice::from_raw_parts(
                                frame.data[1] as *const _,
                                frame.linesize[1] as usize * frame.height as usize,
                            )
                        },
                        &[],
                    ],
                };

                let texture = SoftwareTexture {
                    buffers: &buffers,
                    size: Size {
                        width: frame.width,
                        height: frame.height,
                    },
                };

                self.renderer.submit(match frame.format {
                    VideoFormat::RGBA => Texture::Rgba(TextureResource::Buffer(texture)),
                    VideoFormat::NV12 => Texture::Nv12(TextureResource::Buffer(texture)),
                })?;
            };

            Ok(())
        }
    }
}

#[cfg(not(feature = "wgpu"))]
#[cfg(target_os = "windows")]
pub mod win32 {
    use anyhow::{anyhow, Result};
    use frame::{Resource, VideoFrame, VideoSize, VideoTransform, VideoTransformDescriptor};
    use utils::win32::windows::Win32::{
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
    use utils::win32::{d3d_texture_borrowed_raw, Direct3DDevice};

    use crate::Window;

    pub struct VideoPlayer {
        direct3d: Direct3DDevice,
        swap_chain: IDXGISwapChain,
        render_target_view: ID3D11RenderTargetView,
        video_processor: Option<VideoTransform>,
    }

    unsafe impl Send for VideoPlayer {}
    unsafe impl Sync for VideoPlayer {}

    impl VideoPlayer {
        pub fn new(window: Window) -> Result<Self> {
            let size = window.size()?;
            log::info!("renderer: create video render, size={:?}", size);

            let direct3d = crate::DIRECT_3D_DEVICE
                .read()
                .unwrap()
                .clone()
                .ok_or_else(|| anyhow!("not found a direct3d device"))?;
            let swap_chain = unsafe {
                let dxgi_factory = CreateDXGIFactory::<IDXGIFactory>()?;

                let mut desc = DXGI_SWAP_CHAIN_DESC::default();
                desc.BufferCount = 1;
                desc.BufferDesc.Width = size.width;
                desc.BufferDesc.Height = size.height;
                desc.BufferDesc.Format = DXGI_FORMAT_R8G8B8A8_UNORM;
                desc.BufferUsage = DXGI_USAGE_RENDER_TARGET_OUTPUT;
                desc.OutputWindow = HWND(window.0 as *mut _);
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
                vp.Width = size.width as f32;
                vp.Height = size.height as f32;
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
                self.video_processor = Some(VideoTransform::new(VideoTransformDescriptor {
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
