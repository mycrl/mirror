use anyhow::Result;
use common::frame::VideoFrame;
use pixels::{
    raw_window_handle::{
        HasRawDisplayHandle, HasRawWindowHandle, RawDisplayHandle, RawWindowHandle,
        Win32WindowHandle, WindowsDisplayHandle,
    },
    wgpu::{Backends, TextureFormat},
    Pixels, PixelsBuilder, SurfaceTexture,
};

#[derive(Debug, Clone, Copy)]
pub struct Size {
    pub width: u32,
    pub height: u32,
}

pub enum WindowHandle {
    Win32(Win32WindowHandle),
}

unsafe impl HasRawWindowHandle for WindowHandle {
    fn raw_window_handle(&self) -> RawWindowHandle {
        match self {
            Self::Win32(handle) => RawWindowHandle::Win32(handle.clone()),
        }
    }
}

unsafe impl HasRawDisplayHandle for WindowHandle {
    fn raw_display_handle(&self) -> RawDisplayHandle {
        RawDisplayHandle::Windows(WindowsDisplayHandle::empty())
    }
}

pub struct VideoRender {
    buffer: Vec<u8>,
    pixels: Pixels,
    size: Size,
}

impl VideoRender {
    pub fn new(size: Size, window: &WindowHandle) -> Result<Self> {
        Ok(Self {
            size,
            buffer: vec![0u8; size.width as usize * size.height as usize * 4],
            pixels: PixelsBuilder::new(
                size.width,
                size.height,
                SurfaceTexture::new(size.width, size.height, window),
            )
            .surface_texture_format(TextureFormat::Rgba8UnormSrgb)
            .wgpu_backend(Backends::DX12)
            .build()?,
        })
    }

    pub fn send(&mut self, frame: &VideoFrame) -> Result<()> {
        Ok(())
    }

    pub fn resize(&mut self, size: Size) -> Result<()> {
        self.pixels.resize_surface(size.width, size.height)?;
        Ok(())
    }
}
