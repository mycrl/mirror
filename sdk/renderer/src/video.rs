use std::{
    ffi::{c_int, c_void},
    ptr::{null, null_mut},
};

use anyhow::{anyhow, Result};
use common::{frame::VideoFrame, strings::Strings};
use sdl2::sys::{
    SDL_CreateRenderer, SDL_CreateTexture, SDL_CreateWindowFrom, SDL_DestroyRenderer,
    SDL_DestroyTexture, SDL_DestroyWindow, SDL_GetError, SDL_GetRendererInfo, SDL_Init,
    SDL_PixelFormatEnum, SDL_Quit, SDL_Rect, SDL_RenderClear, SDL_RenderCopyEx, SDL_RenderPresent,
    SDL_Renderer, SDL_RendererFlip, SDL_RendererInfo, SDL_Texture, SDL_TextureAccess,
    SDL_UpdateNVTexture, SDL_Window, SDL_INIT_VIDEO,
};

#[derive(Debug, Clone, Copy)]
pub struct Size {
    pub width: u32,
    pub height: u32,
}

#[allow(unused)]
pub enum WindowHandle {
    Win32(*mut c_void),
}

pub struct VideoRender {
    window: *mut SDL_Window,
    renderer: *mut SDL_Renderer,
    texture: *mut SDL_Texture,
    rect: SDL_Rect,
}

unsafe impl Send for VideoRender {}
unsafe impl Sync for VideoRender {}

impl VideoRender {
    pub fn new(size: Size, handle: &WindowHandle) -> Result<Self> {
        log::info!("renderer: create video render, size={:?}", size);

        if unsafe { SDL_Init(SDL_INIT_VIDEO) } != 0 {
            return error();
        }

        let window = unsafe {
            SDL_CreateWindowFrom(match handle {
                WindowHandle::Win32(hwnd) => *hwnd,
            })
        };

        if window.is_null() {
            return error();
        }

        let renderer = unsafe {
            SDL_CreateRenderer(
                window,
                -1,
                0x00000002 /* SDL_RENDERER_ACCELERATED */ | 0x00000004, /* SDL_RENDERER_PRESENTVSYNC */
            )
        };

        if renderer.is_null() {
            return error();
        }

        {
            let mut info = SDL_RendererInfo {
                name: null(),
                flags: 0,
                num_texture_formats: 0,
                texture_formats: [0; 16],
                max_texture_height: 0,
                max_texture_width: 0,
            };

            if unsafe { SDL_GetRendererInfo(renderer, &mut info) } == 0 {
                if let Ok(name) = Strings::from(info.name).to_string() {
                    log::info!("renderer: video render use: {}", name);
                }
            }
        }

        Ok(Self {
            window,
            renderer,
            texture: null_mut(),
            rect: SDL_Rect {
                w: size.width as c_int,
                h: size.height as c_int,
                x: 0,
                y: 0,
            },
        })
    }

    /// Draw this pixel buffer to the configured [`SurfaceTexture`].
    ///
    /// # Errors
    ///
    /// Returns an error when [`wgpu::Surface::get_current_texture`] fails.
    pub fn send(&mut self, frame: &VideoFrame) -> Result<()> {
        if self.texture.is_null() {
            self.texture = unsafe {
                SDL_CreateTexture(
                    self.renderer,
                    SDL_PixelFormatEnum::SDL_PIXELFORMAT_NV12 as u32,
                    SDL_TextureAccess::SDL_TEXTUREACCESS_STREAMING as c_int,
                    frame.width as c_int,
                    frame.height as c_int,
                )
            };

            if self.texture.is_null() {
                return error();
            }
        }

        if unsafe {
            SDL_UpdateNVTexture(
                self.texture,
                null(),
                frame.data[0],
                frame.linesize[0] as _,
                frame.data[1],
                frame.linesize[1] as _,
            )
        } != 0
        {
            return error();
        }

        if unsafe { SDL_RenderClear(self.renderer) } != 0 {
            return error();
        }

        if unsafe {
            SDL_RenderCopyEx(
                self.renderer,
                self.texture,
                null(),
                &self.rect,
                0.0,
                null(),
                SDL_RendererFlip::SDL_FLIP_NONE,
            )
        } != 0
        {
            return error();
        }

        unsafe { SDL_RenderPresent(self.renderer) }
        Ok(())
    }
}

impl Drop for VideoRender {
    fn drop(&mut self) {
        if !self.texture.is_null() {
            unsafe { SDL_DestroyTexture(self.texture) }
        }

        unsafe { SDL_DestroyRenderer(self.renderer) }
        unsafe { SDL_DestroyWindow(self.window) }
        unsafe { SDL_Quit() }
    }
}

fn error<T>() -> Result<T> {
    Err(anyhow!(
        "{:?}",
        Strings::from(unsafe { SDL_GetError() }).to_string()
    ))
}
