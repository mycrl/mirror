use std::{ffi::c_void, fmt::Debug, ptr::null_mut};

use common::{logger, win32::windows::Win32::Foundation::HWND, Size};
use mirror::{
    raw_window_handle::{
        DisplayHandle, HandleError, HasDisplayHandle, HasWindowHandle, RawWindowHandle,
        Win32WindowHandle, WindowHandle,
    },
    AVFrameSink, AudioFrame, VideoFrame,
};

/// Windows yes! The Windows dynamic library has an entry, so just
/// initialize the logger and set the process priority at the entry.
#[no_mangle]
#[allow(non_snake_case)]
extern "system" fn DllMain(
    _module: u32,
    call_reason: usize,
    _reserved: *const std::ffi::c_void,
) -> bool {
    match call_reason {
            1 /* DLL_PROCESS_ATTACH */ => {
                let _ = logger::init(log::LevelFilter::Info, None);
                std::panic::set_hook(Box::new(|info| {
                    log::error!(
                        "pnaic: location={:?}, message={:?}",
                        info.location(),
                        info.payload().downcast_ref::<String>(),
                    );
                }));

                true
            },
            _ => true,
        }
}

// In fact, this is a package that is convenient for recording errors. If the
// result is an error message, it is output to the log. This function does not
// make any changes to the result.
#[inline]
fn checker<T, E: Debug>(result: Result<T, E>) -> Result<T, E> {
    if let Err(e) = &result {
        log::error!("{:?}", e);

        if cfg!(debug_assertions) {
            println!("{:#?}", e);
        }
    }

    result
}

#[repr(C)]
pub enum VideoRenderBackend {
    Dx11,
    Wgpu,
}

impl Into<mirror::VideoRenderBackend> for VideoRenderBackend {
    fn into(self) -> mirror::VideoRenderBackend {
        match self {
            Self::Dx11 => mirror::VideoRenderBackend::Dx11,
            Self::Wgpu => mirror::VideoRenderBackend::Wgpu,
        }
    }
}

/// A window handle for a particular windowing system.
#[repr(C)]
#[derive(Debug, Clone)]
pub enum Window {
    Win32(HWND, Size),
}

unsafe impl Send for Window {}
unsafe impl Sync for Window {}

impl Window {
    fn size(&self) -> Size {
        *match self {
            Self::Win32(_, size) => size,
        }
    }
}

impl HasDisplayHandle for Window {
    fn display_handle(&self) -> Result<DisplayHandle<'_>, HandleError> {
        Ok(match self {
            Self::Win32(_, _) => DisplayHandle::windows(),
        })
    }
}

impl HasWindowHandle for Window {
    fn window_handle(&self) -> Result<WindowHandle<'_>, HandleError> {
        Ok(match self {
            Self::Win32(hwnd, _) => unsafe {
                WindowHandle::borrow_raw(RawWindowHandle::Win32(Win32WindowHandle::new(
                    std::num::NonZeroIsize::new(hwnd.0 as isize).unwrap(),
                )))
            },
        })
    }
}

/// Raw window handle for Win32.
///
/// This variant is used on Windows systems.
#[no_mangle]
extern "C" fn create_window_handle_for_win32(
    hwnd: *mut c_void,
    width: u32,
    height: u32,
) -> *mut Window {
    Box::into_raw(Box::new(Window::Win32(HWND(hwnd), Size { width, height })))
}

/// Destroy the window handle.
#[no_mangle]
extern "C" fn window_handle_destroy(window_handle: *mut Window) {
    assert!(!window_handle.is_null());

    drop(unsafe { Box::from_raw(window_handle) });
}

#[repr(C)]
struct RawRenderer(mirror::Render<'static>);

/// Creating a window renderer.
#[no_mangle]
#[allow(unused_variables)]
extern "C" fn renderer_create(
    window_handle: *const Window,
    backend: VideoRenderBackend,
) -> *mut RawRenderer {
    assert!(!window_handle.is_null());

    let window = unsafe { &*window_handle };

    let func = || {
        Ok::<RawRenderer, anyhow::Error>(RawRenderer(mirror::Render::new(
            backend.into(),
            window,
            window.size(),
        )?))
    };

    checker(func())
        .map(|ret| Box::into_raw(Box::new(ret)))
        .unwrap_or_else(|_| null_mut())
}

/// Push the video frame into the renderer, which will update the window
/// texture.
#[no_mangle]
extern "C" fn renderer_on_video(render: *mut RawRenderer, frame: *const VideoFrame) -> bool {
    assert!(!render.is_null() && !frame.is_null());

    unsafe { &*render }.0.video(unsafe { &*frame })
}

/// Push the audio frame into the renderer, which will append to audio
/// queue.
#[no_mangle]
extern "C" fn renderer_on_audio(render: *mut RawRenderer, frame: *const AudioFrame) -> bool {
    assert!(!render.is_null() && !frame.is_null());

    unsafe { &*render }.0.audio(unsafe { &*frame })
}

/// Destroy the window renderer.
#[no_mangle]
extern "C" fn renderer_destroy(render: *mut RawRenderer) {
    assert!(!render.is_null());

    drop(unsafe { Box::from_raw(render) });
}
