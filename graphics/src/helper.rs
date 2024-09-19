#[cfg(target_os = "windows")]
pub mod win32 {
    use std::ptr::null_mut;

    use thiserror::Error;
    use utils::win32::{
        EasyTexture, ID3D11Texture2D, ID3D12Resource, Interface, DXGI_FORMAT_NV12,
        DXGI_FORMAT_R8G8B8A8_UNORM,
    };
    use wgpu::{
        hal::api::Dx12, Device, Extent3d, Texture, TextureDescriptor, TextureDimension,
        TextureFormat, TextureUsages,
    };

    #[derive(Debug, Error)]
    pub enum FromDxgiResourceError {
        #[error(transparent)]
        GetSharedError(#[from] utils::win32::Error),
        #[error("not found wgpu dx12 device")]
        NotFoundDxBackend,
        #[error("unable to open dx12 shared handle")]
        NotOpenDxSharedHandle,
    }

    pub fn create_texture_from_dx11_texture(
        device: &Device,
        texture: &ID3D11Texture2D,
    ) -> Result<Texture, FromDxgiResourceError> {
        let desc = texture.desc();
        let resource = unsafe {
            let handle = texture.get_shared()?;

            device
                .as_hal::<Dx12, _, _>(|hdevice| {
                    hdevice.map(|hdevice| {
                        let raw_device = hdevice.raw_device();

                        let mut resource = null_mut();
                        if raw_device.OpenSharedHandle(
                            handle.0,
                            std::mem::transmute(&ID3D12Resource::IID),
                            &mut resource,
                        ) == 0
                        {
                            Some(resource)
                        } else {
                            None
                        }
                    })
                })
                .ok_or_else(|| FromDxgiResourceError::NotFoundDxBackend)?
                .ok_or_else(|| FromDxgiResourceError::NotFoundDxBackend)?
                .ok_or_else(|| FromDxgiResourceError::NotOpenDxSharedHandle)?
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
