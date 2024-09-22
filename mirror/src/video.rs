use crate::Window;

use anyhow::{anyhow, Result};
use frame::{VideoFormat, VideoFrame};
use graphics::{HardwareTexture, SoftwareTexture, Texture, TextureResource};
use utils::Size;

#[cfg(target_os = "windows")]
use utils::win32::{d3d_texture_borrowed_raw, EasyTexture};

#[cfg(all(feature = "dx11", target_os = "windows"))]
use graphics::dx11::Dx11Renderer;

#[cfg(not(feature = "dx11"))]
use graphics::Renderer;

pub struct VideoPlayer {
    #[cfg(not(feature = "dx11"))]
    renderer: Renderer<'static>,
    #[cfg(all(feature = "dx11", target_os = "windows"))]
    renderer: Dx11Renderer,
}

impl VideoPlayer {
    pub fn new(window: Window) -> Result<Self> {
        let size = window.size()?;
        Ok(Self {
            #[cfg(not(feature = "dx11"))]
            renderer: Renderer::new(window, size)?,
            #[cfg(all(feature = "dx11", target_os = "windows"))]
            renderer: Dx11Renderer::new(
                window.raw(),
                size,
                crate::DIRECT_3D_DEVICE.read().unwrap().clone().unwrap(),
            )?,
        })
    }

    pub fn send(&mut self, frame: &VideoFrame) -> Result<()> {
        if frame.hardware {
            #[cfg(target_os = "windows")]
            {
                let dx_tex = d3d_texture_borrowed_raw(&(frame.data[0] as *mut _))
                    .cloned()
                    .ok_or_else(|| anyhow!("not found a texture"))?;

                let desc = dx_tex.desc();
                let texture = TextureResource::Texture(HardwareTexture::Dx11(
                    &dx_tex,
                    &desc,
                    frame.data[1] as u32,
                ));

                self.renderer.submit(match frame.format {
                    VideoFormat::RGBA => Texture::Rgba(texture),
                    VideoFormat::NV12 => Texture::Nv12(texture),
                    VideoFormat::I420 => unimplemented!("no hardware texture for I420"),
                })?;
            }
        } else {
            let buffers = match frame.format {
                // RGBA stands for red green blue alpha. While it is sometimes described as a
                // color space, it is actually a three-channel RGB color model supplemented
                // with a fourth alpha channel. Alpha indicates how opaque each pixel is and
                // allows an image to be combined over others using alpha compositing, with
                // transparent areas and anti-aliasing of the edges of opaque regions. Each
                // pixel is a 4D vector.
                //
                // The term does not define what RGB color space is being used. It also does
                // not state whether or not the colors are premultiplied by the alpha value,
                // and if they are it does not state what color space that premultiplication
                // was done in. This means more information than just "RGBA" is needed to
                // determine how to handle an image.
                //
                // In some contexts the abbreviation "RGBA" means a specific memory layout
                // (called RGBA8888 below), with other terms such as "BGRA" used for
                // alternatives. In other contexts "RGBA" means any layout.
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
                // YCbCr, Y′CbCr, or Y Pb/Cb Pr/Cr, also written as YCBCR or Y′CBCR, is a
                // family of color spaces used as a part of the color image pipeline in video
                // and digital photography systems. Y′ is the luma component and CB and CR are
                // the blue-difference and red-difference chroma components. Y′ (with prime) is
                // distinguished from Y, which is luminance, meaning that light intensity is
                // nonlinearly encoded based on gamma corrected RGB primaries.
                //
                // Y′CbCr color spaces are defined by a mathematical coordinate transformation
                // from an associated RGB primaries and white point. If the underlying RGB
                // color space is absolute, the Y′CbCr color space is an absolute color space
                // as well; conversely, if the RGB space is ill-defined, so is Y′CbCr. The
                // transformation is defined in equations 32, 33 in ITU-T H.273. Nevertheless
                // that rule does not apply to P3-D65 primaries used by Netflix with
                // BT.2020-NCL matrix, so that means matrix was not derived from primaries, but
                // now Netflix allows BT.2020 primaries (since 2021).[1] The same happens with
                // JPEG: it has BT.601 matrix derived from System M primaries, yet the
                // primaries of most images are BT.709.
                VideoFormat::I420 => [
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
                    unsafe {
                        std::slice::from_raw_parts(
                            frame.data[2] as *const _,
                            frame.linesize[2] as usize * frame.height as usize,
                        )
                    },
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
                VideoFormat::I420 => Texture::I420(texture),
            })?;
        };

        Ok(())
    }
}
