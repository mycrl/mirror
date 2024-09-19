use crate::Window;

use anyhow::{anyhow, Result};
use frame::{VideoFormat, VideoFrame};
use graphics::{HardwareTexture, Renderer, SoftwareTexture, Texture, TextureResource};
use utils::{win32::d3d_texture_borrowed_raw, Size};

pub struct VideoPlayer(Renderer<'static>);

impl VideoPlayer {
    pub fn new(window: Window) -> Result<Self> {
        let size = window.size()?;
        Ok(Self(Renderer::new(window, size)?))
    }

    pub fn send(&mut self, frame: &VideoFrame) -> Result<()> {
        if frame.hardware {
            let dx_tex = d3d_texture_borrowed_raw(&(frame.data[0] as *mut _))
                .cloned()
                .ok_or_else(|| anyhow!("not found a texture"))?;

            let texture = TextureResource::Texture(HardwareTexture::Dx11(&dx_tex));
            self.0.submit(match frame.format {
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

            self.0.submit(match frame.format {
                VideoFormat::RGBA => Texture::Rgba(TextureResource::Buffer(texture)),
                VideoFormat::NV12 => Texture::Nv12(TextureResource::Buffer(texture)),
            })?;
        };

        Ok(())
    }
}
