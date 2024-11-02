use super::Texture2DSample;

use hylarana_common::Size;
use wgpu::{Device, Texture, TextureAspect, TextureFormat};

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
pub struct I420(Texture, Texture, Texture);

impl I420 {
    pub(crate) fn new(device: &Device, size: Size) -> Self {
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
        _: Option<&'a Texture>,
    ) -> impl IntoIterator<Item = (&'a Texture, TextureFormat, TextureAspect)> {
        [
            (&self.0, TextureFormat::R8Unorm, TextureAspect::All),
            (&self.1, TextureFormat::R8Unorm, TextureAspect::All),
            (&self.2, TextureFormat::R8Unorm, TextureAspect::All),
        ]
    }

    fn copy_buffer_descriptors<'a>(
        &self,
        buffers: &'a [&'a [u8]],
    ) -> impl IntoIterator<Item = (&'a [u8], &Texture, TextureAspect, Size)> {
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
