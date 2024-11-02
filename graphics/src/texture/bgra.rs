use super::Texture2DSample;

use hylarana_common::Size;
use wgpu::{Device, Texture, TextureAspect, TextureFormat};

pub struct Bgra(Texture);

impl Bgra {
    pub(crate) fn new(device: &Device, size: Size) -> Self {
        Self(Self::create(device, size).next().unwrap())
    }
}

impl Texture2DSample for Bgra {
    fn create_texture_descriptor(size: Size) -> impl IntoIterator<Item = (Size, TextureFormat)> {
        [(size, TextureFormat::Bgra8Unorm)]
    }

    fn views_descriptors<'a>(
        &'a self,
        texture: Option<&'a Texture>,
    ) -> impl IntoIterator<Item = (&'a Texture, TextureFormat, TextureAspect)> {
        [(
            texture.unwrap_or_else(|| &self.0),
            TextureFormat::Bgra8Unorm,
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
