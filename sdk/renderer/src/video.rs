use std::{ffi::c_int, sync::{atomic::AtomicU32, Mutex}};

use anyhow::Result;
use common::{atomic::EasyAtomic, frame::VideoFrame};
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

struct AtomicSize {
    width: AtomicU32,
    height: AtomicU32,
}

impl Into<AtomicSize> for Size {
    fn into(self) -> AtomicSize {
        AtomicSize {
            width: AtomicU32::new(self.width),
            height: AtomicU32::new(self.height),
        }
    }
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
    pixels: Mutex<Pixels>,
    size: AtomicSize,
}

impl VideoRender {
    pub fn new(size: Size, window: &WindowHandle) -> Result<Self> {
        Ok(Self {
            size: size.into(),
            pixels: Mutex::new(
                PixelsBuilder::new(
                    size.width,
                    size.height,
                    SurfaceTexture::new(size.width, size.height, window),
                )
                .surface_texture_format(TextureFormat::Rgba8UnormSrgb)
                .wgpu_backend(Backends::DX12)
                .build()?,
            ),
        })
    }

    pub fn send(&self, frame: &VideoFrame) -> Result<()> {
        let mut pixels = self.pixels.lock().unwrap();

        {
            if self.size.width.get() != frame.rect.width as u32 || self.size.height.get() != frame.rect.height as u32 {
                pixels.resize_buffer(frame.rect.width as u32, frame.rect.height as u32)?;
                self.size.height.update(frame.rect.height as u32);
                self.size.width.update(frame.rect.width as u32);
            }
        }

        let texture = pixels.frame_mut();
        // unsafe {
        //     libyuv::nv12_to_argb(
        //         frame.data[0],
        //         frame.linesize[0] as c_int,
        //         frame.data[1],
        //         frame.linesize[1] as c_int,
        //         texture.as_mut_ptr(),
        //         frame.rect.width as c_int * 4,
        //         frame.rect.width as c_int,
        //         frame.rect.height as c_int,
        //     );
        // }

        pixels.render()?;
        Ok(())
    }

    pub fn resize(&self, size: Size) -> Result<()> {
        self.pixels
            .lock()
            .unwrap()
            .resize_surface(size.width, size.height)?;
        Ok(())
    }
}
