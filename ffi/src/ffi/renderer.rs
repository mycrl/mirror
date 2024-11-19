use std::{
    ffi::{c_int, c_void},
    ptr::{null_mut, NonNull},
};

use hylarana::{
    raw_window_handle::{
        AppKitWindowHandle, DisplayHandle, HandleError, HasDisplayHandle, HasWindowHandle,
        RawDisplayHandle, RawWindowHandle, WaylandDisplayHandle, WaylandWindowHandle,
        Win32WindowHandle, WindowHandle, XcbDisplayHandle, XcbWindowHandle, XlibDisplayHandle,
        XlibWindowHandle,
    },
    AVFrameSink, AudioFrame, Renderer, Size, VideoFrame,
};

use crate::ffi::log_error;

use super::RawGraphicsBackend;

/// A window handle for a particular windowing system.
#[repr(C)]
#[derive(Clone)]
#[allow(unused)]
enum RawWindowHandleRef {
    Win32(*mut c_void, Size),
    Xlib(u32, *mut c_void, c_int, Size),
    Xcb(u32, *mut c_void, c_int, Size),
    Wayland(*mut c_void, *mut c_void, Size),
    AppKit(*mut c_void, Size),
}

unsafe impl Send for RawWindowHandleRef {}
unsafe impl Sync for RawWindowHandleRef {}

impl RawWindowHandleRef {
    fn size(&self) -> Size {
        *match self {
            Self::Win32(_, size)
            | Self::Xlib(_, _, _, size)
            | Self::Xcb(_, _, _, size)
            | Self::Wayland(_, _, size)
            | Self::AppKit(_, size) => size,
        }
    }
}

impl HasDisplayHandle for RawWindowHandleRef {
    fn display_handle(&self) -> Result<DisplayHandle<'_>, HandleError> {
        Ok(match self {
            Self::AppKit(_, _) => DisplayHandle::appkit(),
            Self::Win32(_, _) => DisplayHandle::windows(),
            // This variant is likely to show up anywhere someone manages to get X11 working
            // that Xlib can be built for, which is to say, most (but not all) Unix systems.
            Self::Xlib(_, display, screen, _) => unsafe {
                DisplayHandle::borrow_raw(RawDisplayHandle::Xlib(XlibDisplayHandle::new(
                    NonNull::new(*display),
                    *screen,
                )))
            },
            // This variant is likely to show up anywhere someone manages to get X11 working
            // that XCB can be built for, which is to say, most (but not all) Unix systems.
            Self::Xcb(_, display, screen, _) => unsafe {
                DisplayHandle::borrow_raw(RawDisplayHandle::Xcb(XcbDisplayHandle::new(
                    NonNull::new(*display),
                    *screen,
                )))
            },
            // This variant should be expected anywhere Wayland works, which is currently some
            // subset of unix systems.
            Self::Wayland(_, display, _) => unsafe {
                DisplayHandle::borrow_raw(RawDisplayHandle::Wayland(WaylandDisplayHandle::new(
                    NonNull::new(*display).unwrap(),
                )))
            },
        })
    }
}

impl HasWindowHandle for RawWindowHandleRef {
    fn window_handle(&self) -> Result<WindowHandle<'_>, HandleError> {
        Ok(match self {
            // This variant is used on Windows systems.
            Self::Win32(window, _) => unsafe {
                WindowHandle::borrow_raw(RawWindowHandle::Win32(Win32WindowHandle::new(
                    std::num::NonZeroIsize::new(*window as isize).unwrap(),
                )))
            },
            // This variant is likely to show up anywhere someone manages to get X11
            // working that Xlib can be built for, which is to say, most (but not all)
            // Unix systems.
            Self::Xlib(window, _, _, _) => unsafe {
                WindowHandle::borrow_raw(RawWindowHandle::Xlib(XlibWindowHandle::new(
                    (*window).into(),
                )))
            },
            // This variant is likely to show up anywhere someone manages to get X11
            // working that XCB can be built for, which is to say, most (but not all)
            // Unix systems.
            Self::Xcb(window, _, _, _) => unsafe {
                WindowHandle::borrow_raw(RawWindowHandle::Xcb(XcbWindowHandle::new(
                    std::num::NonZeroU32::new(*window).unwrap(),
                )))
            },
            // This variant should be expected anywhere Wayland works, which is
            // currently some subset of unix systems.
            Self::Wayland(surface, _, _) => unsafe {
                WindowHandle::borrow_raw(RawWindowHandle::Wayland(WaylandWindowHandle::new(
                    std::ptr::NonNull::new_unchecked(*surface),
                )))
            },
            // This variant is likely to be used on macOS, although Mac Catalyst
            // ($arch-apple-ios-macabi targets, which can notably use UIKit or AppKit) can also
            // use it despite being target_os = "ios".
            Self::AppKit(window, _) => unsafe {
                WindowHandle::borrow_raw(RawWindowHandle::AppKit(AppKitWindowHandle::new(
                    std::ptr::NonNull::new_unchecked(*window),
                )))
            },
        })
    }
}

/// Raw window handle for Win32.
#[no_mangle]
#[cfg(target_os = "windows")]
extern "C" fn hylarana_create_window_handle_for_win32(
    hwnd: *mut c_void,
    width: u32,
    height: u32,
) -> *mut RawWindowHandleRef {
    Box::into_raw(Box::new(RawWindowHandleRef::Win32(
        hwnd,
        Size { width, height },
    )))
}

/// A raw window handle for Xlib.
#[no_mangle]
#[cfg(target_os = "linux")]
extern "C" fn hylarana_create_window_handle_for_xlib(
    hwnd: u32,
    display: *mut c_void,
    screen: c_int,
    width: u32,
    height: u32,
) -> *mut RawWindowHandleRef {
    Box::into_raw(Box::new(RawWindowHandleRef::Xlib(
        hwnd,
        display,
        screen,
        Size { width, height },
    )))
}

/// A raw window handle for Xcb.
#[no_mangle]
#[cfg(target_os = "linux")]
extern "C" fn hylarana_create_window_handle_for_xcb(
    hwnd: u32,
    display: *mut c_void,
    screen: c_int,
    width: u32,
    height: u32,
) -> *mut RawWindowHandleRef {
    Box::into_raw(Box::new(RawWindowHandleRef::Xcb(
        hwnd,
        display,
        screen,
        Size { width, height },
    )))
}

/// A raw window handle for Wayland.
#[no_mangle]
#[cfg(target_os = "linux")]
extern "C" fn hylarana_create_window_handle_for_wayland(
    hwnd: *mut std::ffi::c_void,
    display: *mut c_void,
    width: u32,
    height: u32,
) -> *mut RawWindowHandleRef {
    Box::into_raw(Box::new(RawWindowHandleRef::Wayland(
        hwnd,
        display,
        Size { width, height },
    )))
}

/// A raw window handle for AppKit.
#[no_mangle]
#[cfg(target_os = "macos")]
extern "C" fn hylarana_create_window_handle_for_appkit(
    hwnd: *mut std::ffi::c_void,
    width: u32,
    height: u32,
) -> *mut RawWindowHandleRef {
    Box::into_raw(Box::new(RawWindowHandleRef::AppKit(
        hwnd,
        Size { width, height },
    )))
}

/// Destroy the window handle.
#[no_mangle]
extern "C" fn hylarana_window_handle_destroy(window_handle: *mut RawWindowHandleRef) {
    assert!(!window_handle.is_null());

    drop(unsafe { Box::from_raw(window_handle) });
}

#[repr(C)]
struct RawRenderer(Renderer<'static>);

/// Creating a window renderer.
#[no_mangle]
extern "C" fn hylarana_renderer_create(
    window_handle: *const RawWindowHandleRef,
    backend: RawGraphicsBackend,
) -> *mut RawRenderer {
    assert!(!window_handle.is_null());

    let window = unsafe { &*window_handle };
    let func = || {
        Ok::<RawRenderer, anyhow::Error>(RawRenderer(Renderer::new(
            backend.into(),
            window,
            window.size(),
        )?))
    };

    log_error(func())
        .map(|ret| Box::into_raw(Box::new(ret)))
        .unwrap_or_else(|_| null_mut())
}

/// Push the video frame into the renderer, which will update the window
/// texture.
#[no_mangle]
extern "C" fn hylarana_renderer_on_video(
    render: *mut RawRenderer,
    frame: *const VideoFrame,
) -> bool {
    assert!(!render.is_null());
    assert!(!frame.is_null());

    unsafe { &*render }.0.video(unsafe { &*frame })
}

/// Push the audio frame into the renderer, which will append to audio
/// queue.
#[no_mangle]
extern "C" fn hylarana_renderer_on_audio(
    render: *mut RawRenderer,
    frame: *const AudioFrame,
) -> bool {
    assert!(!render.is_null());
    assert!(!frame.is_null());

    unsafe { &*render }.0.audio(unsafe { &*frame })
}

/// Destroy the window renderer.
#[no_mangle]
extern "C" fn hylarana_renderer_destroy(render: *mut RawRenderer) {
    assert!(!render.is_null());

    drop(unsafe { Box::from_raw(render) });
}
