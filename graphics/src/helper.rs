#[cfg(target_os = "windows")]
pub mod win32 {
    use std::sync::Arc;

    use thiserror::Error;
    use utils::win32::{
        windows::Win32::Graphics::{
            Direct3D11::{
                ID3D11Texture2D, D3D11_RESOURCE_MISC_SHARED, D3D11_TEXTURE2D_DESC,
                D3D11_USAGE_DEFAULT,
            },
            Direct3D12::ID3D12Resource,
            Dxgi::Common::{DXGI_FORMAT_NV12, DXGI_FORMAT_R8G8B8A8_UNORM, DXGI_SAMPLE_DESC},
        },
        Direct3DDevice, EasyTexture,
    };

    use wgpu::{
        hal::api::Dx12, Device, Extent3d, Texture, TextureDescriptor, TextureDimension,
        TextureFormat, TextureUsages,
    };

    #[derive(Debug, Error)]
    pub enum Dx11OnWgpuCompatibilityLayerError {
        #[error(transparent)]
        DxError(#[from] utils::win32::windows::core::Error),
        #[error("not found wgpu dx12 device")]
        NotFoundDxBackend,
        #[error("dx11 shared handle is invalid")]
        InvalidDxSharedHandle,
    }

    pub struct Dx11OnWgpuCompatibilityLayer {
        device: Arc<Device>,
        direct3d: Direct3DDevice,
        texture: Option<ID3D11Texture2D>,
    }

    unsafe impl Sync for Dx11OnWgpuCompatibilityLayer {}
    unsafe impl Send for Dx11OnWgpuCompatibilityLayer {}

    impl Dx11OnWgpuCompatibilityLayer {
        pub fn new(device: Arc<Device>, direct3d: Direct3DDevice) -> Self {
            Self {
                texture: None,
                direct3d,
                device,
            }
        }

        pub fn texture_from_dx11(
            &mut self,
            texture: &ID3D11Texture2D,
            index: u32,
        ) -> Result<Texture, Dx11OnWgpuCompatibilityLayerError> {
            let mut desc = texture.desc();

            let handle = if desc.MiscFlags & D3D11_RESOURCE_MISC_SHARED.0 as u32 == 0 || index != 0
            {
                desc = D3D11_TEXTURE2D_DESC {
                    Width: desc.Width,
                    Height: desc.Height,
                    MipLevels: 1,
                    ArraySize: 1,
                    Format: desc.Format,
                    SampleDesc: DXGI_SAMPLE_DESC {
                        Count: 1,
                        Quality: 0,
                    },
                    BindFlags: 0,
                    CPUAccessFlags: 0,
                    Usage: D3D11_USAGE_DEFAULT,
                    MiscFlags: D3D11_RESOURCE_MISC_SHARED.0 as u32,
                };

                if self.texture.is_none() {
                    self.texture = Some(unsafe {
                        let mut tex = None;
                        self.direct3d
                            .device
                            .CreateTexture2D(&desc, None, Some(&mut tex))?;
                        tex.unwrap()
                    })
                }

                let dest_tex = self.texture.as_ref().unwrap();
                unsafe {
                    self.direct3d
                        .context
                        .CopySubresourceRegion(dest_tex, 0, 0, 0, 0, texture, index, None);
                }

                dest_tex.get_shared()?
            } else {
                texture.get_shared()?
            };

            if handle.is_invalid() {
                return Err(Dx11OnWgpuCompatibilityLayerError::InvalidDxSharedHandle);
            }

            let resource = unsafe {
                self.device
                    .as_hal::<Dx12, _, _>(|hdevice| {
                        let hdevice = hdevice
                            .ok_or_else(|| Dx11OnWgpuCompatibilityLayerError::NotFoundDxBackend)?;

                        let raw_device = hdevice.raw_device();

                        let mut resource = None::<ID3D12Resource>;
                        raw_device
                            .OpenSharedHandle(handle, &mut resource)
                            .map(|_| resource.unwrap())
                            .map_err(|e| Dx11OnWgpuCompatibilityLayerError::DxError(e))
                    })
                    .ok_or_else(|| Dx11OnWgpuCompatibilityLayerError::NotFoundDxBackend)??
            };

            let desc = TextureDescriptor {
                label: None,
                mip_level_count: desc.MipLevels,
                sample_count: desc.SampleDesc.Count,
                dimension: TextureDimension::D2,
                usage: TextureUsages::TEXTURE_BINDING,
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
                    resource,
                    desc.format,
                    desc.dimension,
                    desc.size,
                    desc.mip_level_count,
                    desc.sample_count,
                );

                self.device.create_texture_from_hal::<Dx12>(texture, &desc)
            })
        }
    }
}
