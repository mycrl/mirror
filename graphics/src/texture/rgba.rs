use super::Texture2DSample;

use hylarana_common::Size;
use wgpu::{Device, Texture, TextureAspect, TextureFormat};

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
pub struct Rgba(Texture);

impl Rgba {
    pub(crate) fn new(device: &Device, size: Size) -> Self {
        Self(Self::create(device, size).next().unwrap())
    }
}

impl Texture2DSample for Rgba {
    fn create_texture_descriptor(size: Size) -> impl IntoIterator<Item = (Size, TextureFormat)> {
        [(size, TextureFormat::Rgba8Unorm)]
    }

    fn views_descriptors<'a>(
        &'a self,
        texture: Option<&'a Texture>,
    ) -> impl IntoIterator<Item = (&'a Texture, TextureFormat, TextureAspect)> {
        [(
            texture.unwrap_or_else(|| &self.0),
            TextureFormat::Rgba8Unorm,
            TextureAspect::All,
        )]
    }

    fn copy_buffer_descriptors<'a>(
        &self,
        buffers: &'a [&'a [u8]],
    ) -> impl IntoIterator<Item = (&'a [u8], &Texture, TextureAspect, Size)> {
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
