use std::{ffi::c_void, fmt::Debug, ptr::null_mut};

use common::{win32::windows::Win32::Foundation::HWND, Size};
use mirror::{
    raw_window_handle::{
        DisplayHandle, HandleError, HasDisplayHandle, HasWindowHandle, RawWindowHandle,
        Win32WindowHandle, WindowHandle,
    },
    AVFrameSink, AudioFrame, VideoFrame,
};

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
    #[cfg(target_os = "windows")]
    Win32(HWND, Size),
    #[cfg(target_os = "linux")]
    Xlib(u64, *mut c_void, c_int, Size),
    #[cfg(target_os = "linux")]
    Xcb(u32, *mut c_void, c_int, Size),
    #[cfg(target_os = "linux")]
    Wayland(*mut c_void, *mut c_void, Size),
}

unsafe impl Send for Window {}
unsafe impl Sync for Window {}

impl Window {
    fn size(&self) -> Size {
        *match self {
            #[cfg(target_os = "windows")]
            Self::Win32(_, size) => size,
            #[cfg(target_os = "linux")]
            Self::Xlib(_, _, _, size) | Self::Xcb(_, _, _, size) | Self::Wayland(_, _, size) => {
                size
            }
        }
    }
}

impl HasDisplayHandle for Window {
    fn display_handle(&self) -> Result<DisplayHandle<'_>, HandleError> {
        Ok(match self {
            #[cfg(target_os = "windows")]
            Self::Win32(_, _) => DisplayHandle::windows(),
            #[cfg(target_os = "linux")]
            Self::Xlib(_, display, screen, _) => unsafe {
                DisplayHandle::borrow_raw(RawDisplayHandle::Xlib(XlibDisplayHandle::new(
                    NonNull::new(*display),
                    *screen,
                )))
            },
            #[cfg(target_os = "linux")]
            Self::Xcb(_, display, screen, _) => unsafe {
                DisplayHandle::borrow_raw(RawDisplayHandle::Xcb(XcbDisplayHandle::new(
                    NonNull::new(*display),
                    *screen,
                )))
            },
            #[cfg(target_os = "linux")]
            Self::Wayland(_, display, _) => unsafe {
                DisplayHandle::borrow_raw(RawDisplayHandle::Wayland(WaylandDisplayHandle::new(
                    NonNull::new(*display).unwrap(),
                )))
            },
        })
    }
}

impl HasWindowHandle for Window {
    fn window_handle(&self) -> Result<WindowHandle<'_>, HandleError> {
        Ok(match self {
            #[cfg(target_os = "windows")]
            Self::Win32(hwnd, _) => unsafe {
                WindowHandle::borrow_raw(RawWindowHandle::Win32(Win32WindowHandle::new(
                    std::num::NonZeroIsize::new(hwnd.0 as isize).unwrap(),
                )))
            },
            #[cfg(target_os = "linux")]
            Self::Xlib(window, _, _, _) => unsafe {
                WindowHandle::borrow_raw(RawWindowHandle::Xlib(XlibWindowHandle::new(*window)))
            },
            #[cfg(target_os = "linux")]
            Self::Xcb(window, _, _, _) => unsafe {
                WindowHandle::borrow_raw(RawWindowHandle::Xcb(XcbWindowHandle::new(
                    std::num::NonZeroU32::new(*window).unwrap(),
                )))
            },
            #[cfg(target_os = "linux")]
            Self::Wayland(surface, _, _) => unsafe {
                WindowHandle::borrow_raw(RawWindowHandle::Wayland(WaylandWindowHandle::new(
                    std::ptr::NonNull::new_unchecked(*surface),
                )))
            },
        })
    }
}

/// Raw window handle for Win32.
///
/// This variant is used on Windows systems.
#[no_mangle]
#[cfg(target_os = "windows")]
extern "C" fn create_window_handle_for_win32(
    hwnd: *mut c_void,
    width: u32,
    height: u32,
) -> *mut Window {
    Box::into_raw(Box::new(Window::Win32(HWND(hwnd), Size { width, height })))
}

/// A raw window handle for Xlib.
///
/// This variant is likely to show up anywhere someone manages to get X11
/// working that Xlib can be built for, which is to say, most (but not all)
/// Unix systems.
#[no_mangle]
#[cfg(target_os = "linux")]
extern "C" fn create_window_handle_for_xlib(
    hwnd: u64,
    display: *mut c_void,
    screen: c_int,
    width: u32,
    height: u32,
) -> *mut Window {
    Box::into_raw(Box::new(Window::Xlib(
        hwnd,
        display,
        screen,
        Size { width, height },
    )))
}

/// A raw window handle for Xcb.
///
/// This variant is likely to show up anywhere someone manages to get X11
/// working that XCB can be built for, which is to say, most (but not all)
/// Unix systems.
#[no_mangle]
#[cfg(target_os = "linux")]
extern "C" fn create_window_handle_for_xcb(
    hwnd: u32,
    display: *mut c_void,
    screen: c_int,
    width: u32,
    height: u32,
) -> *mut Window {
    Box::into_raw(Box::new(Window::Xcb(
        hwnd,
        display,
        screen,
        Size { width, height },
    )))
}

/// A raw window handle for Wayland.
///
/// This variant should be expected anywhere Wayland works, which is
/// currently some subset of unix systems.
#[no_mangle]
#[cfg(target_os = "linux")]
extern "C" fn create_window_handle_for_wayland(
    hwnd: *mut std::ffi::c_void,
    display: *mut c_void,
    width: u32,
    height: u32,
) -> *mut Window {
    Box::into_raw(Box::new(Window::Wayland(
        hwnd,
        display,
        Size { width, height },
    )))
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
