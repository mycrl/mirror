use thiserror::Error;

#[derive(Debug, Error)]
pub enum CompatibilityLayerError {
    #[cfg(target_os = "windows")]
    #[error(transparent)]
    DxError(#[from] common::win32::windows::core::Error),
    #[error("not found wgpu dx12 device")]
    NotFoundDxBackend,
    #[error("dx11 shared handle is invalid")]
    InvalidDxSharedHandle,
}

#[cfg(target_os = "windows")]
pub mod win32 {
    use std::sync::Arc;

    use super::CompatibilityLayerError;

    use common::win32::{
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

    pub struct Dx11OnWgpuCompatibilityLayer {
        device: Arc<Device>,
        direct3d: Direct3DDevice,
        dx_texture: Option<ID3D11Texture2D>,
        texture: Option<Texture>,
    }

    unsafe impl Sync for Dx11OnWgpuCompatibilityLayer {}
    unsafe impl Send for Dx11OnWgpuCompatibilityLayer {}

    impl Dx11OnWgpuCompatibilityLayer {
        pub fn new(device: Arc<Device>, direct3d: Direct3DDevice) -> Self {
            Self {
                dx_texture: None,
                texture: None,
                direct3d,
                device,
            }
        }

        pub fn from_hal(
            &mut self,
            texture: &ID3D11Texture2D,
            index: u32,
        ) -> Result<&Texture, CompatibilityLayerError> {
            // The first texture received, the texture is not initialized yet, initialize
            // the texture here.
            if self.dx_texture.is_none() {
                // Gets the incoming texture properties, the new texture contains only an array
                // of textures and is a shareable texture resource.
                let desc = texture.desc();
                let desc = D3D11_TEXTURE2D_DESC {
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

                // Creates a new texture, which serves as the current texture to be used, and to
                // which external input textures are updated.
                let mut tex = None;
                unsafe {
                    self.direct3d
                        .device
                        .CreateTexture2D(&desc, None, Some(&mut tex))?;
                }

                // Get the texture's shared resources. dx11 textures need to be shared resources
                // if they are to be used by dx12 devices.
                let tex = tex.unwrap();
                let handle = tex.get_shared()?;
                if handle.is_invalid() {
                    return Err(CompatibilityLayerError::InvalidDxSharedHandle);
                } else {
                    self.dx_texture = Some(tex);
                }

                // dx12 device opens dx11 shared resource handle
                let resource = unsafe {
                    self.device
                        .as_hal::<Dx12, _, _>(|hdevice| {
                            let hdevice = hdevice
                                .ok_or_else(|| CompatibilityLayerError::NotFoundDxBackend)?;

                            let raw_device = hdevice.raw_device();

                            let mut resource = None::<ID3D12Resource>;
                            raw_device
                                .OpenSharedHandle(handle, &mut resource)
                                .map(|_| resource.unwrap())
                                .map_err(|e| CompatibilityLayerError::DxError(e))
                        })
                        .ok_or_else(|| CompatibilityLayerError::NotFoundDxBackend)??
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

                // Converts dx12 resources to textures that wgpu can use.
                self.texture = Some(unsafe {
                    let texture = <Dx12 as wgpu::hal::Api>::Device::texture_from_raw(
                        resource,
                        desc.format,
                        desc.dimension,
                        desc.size,
                        desc.mip_level_count,
                        desc.sample_count,
                    );

                    self.device.create_texture_from_hal::<Dx12>(texture, &desc)
                });
            }

            // Copies the input texture to the internal texture.
            if let Some(dest_tex) = self.dx_texture.as_ref() {
                unsafe {
                    self.direct3d
                        .context
                        .CopySubresourceRegion(dest_tex, 0, 0, 0, 0, texture, index, None);
                }
            }

            Ok(self.texture.as_ref().unwrap())
        }
    }
}

#[cfg(target_os = "linux")]
pub mod linux {
    use std::sync::Arc;

    use wgpu::Device;

    pub struct VulkanOnWgpuCompatibilityLayer {}

    impl VulkanOnWgpuCompatibilityLayer {
        pub fn new(_device: Arc<Device>) -> Self {
            Self {}
        }
    }
}

#[cfg(target_os = "macos")]
pub mod linux {
    use std::sync::Arc;

    use wgpu::Device;

    pub struct MetalOnWgpuCompatibilityLayer {}

    impl MetalOnWgpuCompatibilityLayer {
        pub fn new(_device: Arc<Device>) -> Self {
            Self {}
        }
    }
}
