use std::ffi::c_int;

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
        {
            if self.size.width != frame.rect.width as u32
                || self.size.height != frame.rect.height as u32
            {
                self.pixels
                    .resize_buffer(frame.rect.width as u32, frame.rect.height as u32)?;
                self.size.height = frame.rect.height as u32;
                self.size.width = frame.rect.width as u32;
            }
        }

        unsafe {
            libyuv::nv12_to_argb(
                frame.data[0],
                frame.linesize[0] as c_int,
                frame.data[1],
                frame.linesize[1] as c_int,
                self.buffer.as_mut_ptr(),
                frame.rect.width as c_int * 4,
                frame.rect.width as c_int,
                frame.rect.height as c_int,
            );
        }

        unsafe {
            libyuv::argb_to_rgba(
                self.buffer.as_mut_ptr(),
                frame.rect.width as c_int * 4,
                self.pixels.frame_mut().as_mut_ptr(),
                frame.rect.width as c_int * 4,
                frame.rect.width as c_int,
                frame.rect.height as c_int,
            );
        }

        self.pixels.render()?;
        Ok(())
    }

    pub fn resize(&mut self, size: Size) -> Result<()> {
        self.pixels.resize_surface(size.width, size.height)?;
        Ok(())
    }
}
