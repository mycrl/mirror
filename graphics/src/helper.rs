#[cfg(target_os = "windows")]
pub mod win32 {
    use std::ptr::null_mut;

    use thiserror::Error;
    use utils::win32::{
        windows::{
            core::Interface,
            Win32::Graphics::{
                Direct3D11::{ID3D11Texture2D, D3D11_TEXTURE2D_DESC},
                Direct3D12::ID3D12Resource,
                Dxgi::Common::{DXGI_FORMAT_NV12, DXGI_FORMAT_R8G8B8A8_UNORM},
            },
        },
        EasyTexture,
    };
    use wgpu::{
        hal::api::Dx12, Device, Extent3d, Texture, TextureDescriptor, TextureDimension,
        TextureFormat, TextureUsages,
    };

    #[derive(Debug, Error)]
    pub enum FromDxgiResourceError {
        #[error(transparent)]
        GetSharedError(#[from] utils::win32::windows::core::Error),
        #[error("not found wgpu dx12 device")]
        NotFoundDxBackend,
        #[error("unable to open dx12 shared handle")]
        NotOpenDxSharedHandle,
    }

    /// see: https://learn.microsoft.com/en-us/windows/win32/api/d3d11/nn-d3d11-id3d11texture2d
    ///
    /// Get d3d's shared resources and convert them directly to wgpu's texture
    /// type, this interface is meant to use external textures directly.
    pub fn create_texture_from_dx11_texture(
        device: &Device,
        texture: &ID3D11Texture2D,
        desc: &D3D11_TEXTURE2D_DESC,
    ) -> Result<Texture, FromDxgiResourceError> {
        let resource = unsafe {
            let handle = texture.get_shared()?;
            if handle.is_invalid() {
                return Err(FromDxgiResourceError::NotOpenDxSharedHandle);
            }

            device
                .as_hal::<Dx12, _, _>(|hdevice| {
                    let hdevice =
                        hdevice.ok_or_else(|| FromDxgiResourceError::NotFoundDxBackend)?;

                    let raw_device = hdevice.raw_device();
                    let mut resource = null_mut();
                    let ret = raw_device.OpenSharedHandle(
                        handle.0,
                        std::mem::transmute(&ID3D12Resource::IID),
                        &mut resource,
                    );

                    if ret == 0 {
                        Ok(resource)
                    } else {
                        Err(FromDxgiResourceError::NotOpenDxSharedHandle)
                    }
                })
                .ok_or_else(|| FromDxgiResourceError::NotFoundDxBackend)??
        };

        let desc = TextureDescriptor {
            label: None,
            mip_level_count: desc.MipLevels,
            sample_count: desc.SampleDesc.Count,
            dimension: TextureDimension::D2,
            usage: TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_SRC,
            format: match desc.Format {
                DXGI_FORMAT_NV12 => TextureFormat::NV12,
                DXGI_FORMAT_R8G8B8A8_UNORM => TextureFormat::Rgba8Unorm,
                _ => unimplemented!("not supports texture format"),
            },
            view_formats: &[],
            size: Extent3d {
                depth_or_array_layers: desc.ArraySize,
                width: desc.Width,
                height: desc.Height,
            },
        };

        Ok(unsafe {
            let texture = <Dx12 as wgpu::hal::Api>::Device::texture_from_raw(
                d3d12::Resource::from_raw(resource as *mut _),
                desc.format,
                desc.dimension,
                desc.size,
                1,
                1,
            );

            device.create_texture_from_hal::<Dx12>(texture, &desc)
        })
    }
}
