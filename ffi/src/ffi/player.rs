use std::{
    ffi::{c_int, c_ulong, c_void},
    ptr::NonNull,
};

use anyhow::Result;
use hylarana::{
    raw_window_handle::{
        AppKitWindowHandle, DisplayHandle, HandleError, HasDisplayHandle, HasWindowHandle,
        RawDisplayHandle, RawWindowHandle, WaylandDisplayHandle, WaylandWindowHandle,
        Win32WindowHandle, WindowHandle, XlibDisplayHandle, XlibWindowHandle,
    },
    AVFrameObserver, AVFrameStreamPlayer, AVFrameStreamPlayerOptions, Size, SurfaceTarget,
    VideoRenderBackend, VideoRenderOptions,
};

trait GetSize {
    fn size(&self) -> Size;
}

/// A raw window handle for Win32.
///
/// This variant is used on Windows systems.
#[repr(C)]
#[derive(Clone, Copy)]
struct RawWin32Window {
    /// A Win32 HWND handle.
    hwnd: *mut c_void,
    width: u32,
    height: u32,
}

unsafe impl Send for RawWin32Window {}
unsafe impl Sync for RawWin32Window {}

impl GetSize for RawWin32Window {
    fn size(&self) -> Size {
        Size {
            width: self.width,
            height: self.height,
        }
    }
}

impl HasDisplayHandle for RawWin32Window {
    fn display_handle(&self) -> Result<DisplayHandle<'_>, HandleError> {
        Ok(DisplayHandle::windows())
    }
}

impl HasWindowHandle for RawWin32Window {
    fn window_handle(&self) -> Result<WindowHandle<'_>, HandleError> {
        Ok(unsafe {
            WindowHandle::borrow_raw(RawWindowHandle::Win32(Win32WindowHandle::new(
                std::num::NonZeroIsize::new(self.hwnd as isize).unwrap(),
            )))
        })
    }
}

/// A raw window handle for Xlib.
///
/// This variant is likely to show up anywhere someone manages to get X11
/// working that Xlib can be built for, which is to say, most (but not all) Unix
/// systems.
#[repr(C)]
#[derive(Clone, Copy)]
struct RawXlibWindow {
    /// An Xlib Window.
    window: c_ulong,
    /// A pointer to an Xlib Display.
    ///
    /// It is strongly recommended to set this value, however it may be set to
    /// None to request the default display when using EGL.
    display: *mut c_void,
    /// An X11 screen to use with this display handle.
    ///
    /// Note, that X11 could have multiple screens, however graphics APIs could
    /// work only with one screen at the time, given that multiple screens
    /// usually reside on different GPUs.
    screen: c_int,
    width: u32,
    height: u32,
}

unsafe impl Send for RawXlibWindow {}
unsafe impl Sync for RawXlibWindow {}

impl GetSize for RawXlibWindow {
    fn size(&self) -> Size {
        Size {
            width: self.width,
            height: self.height,
        }
    }
}

impl HasDisplayHandle for RawXlibWindow {
    fn display_handle(&self) -> Result<DisplayHandle<'_>, HandleError> {
        Ok(unsafe {
            DisplayHandle::borrow_raw(RawDisplayHandle::Xlib(XlibDisplayHandle::new(
                NonNull::new(self.display),
                self.screen,
            )))
        })
    }
}

impl HasWindowHandle for RawXlibWindow {
    fn window_handle(&self) -> Result<WindowHandle<'_>, HandleError> {
        Ok(unsafe {
            WindowHandle::borrow_raw(RawWindowHandle::Xlib(XlibWindowHandle::new(self.window)))
        })
    }
}

/// A raw window handle for Wayland.
///
/// This variant should be expected anywhere Wayland works, which is currently
/// some subset of unix systems.
#[repr(C)]
#[derive(Clone, Copy)]
struct RawWaylandWindow {
    /// A pointer to a wl_surface.
    surface: *mut c_void,
    /// A pointer to a wl_display.
    display: *mut c_void,
    width: u32,
    height: u32,
}

unsafe impl Send for RawWaylandWindow {}
unsafe impl Sync for RawWaylandWindow {}

impl GetSize for RawWaylandWindow {
    fn size(&self) -> Size {
        Size {
            width: self.width,
            height: self.height,
        }
    }
}

impl HasDisplayHandle for RawWaylandWindow {
    fn display_handle(&self) -> Result<DisplayHandle<'_>, HandleError> {
        Ok(unsafe {
            DisplayHandle::borrow_raw(RawDisplayHandle::Wayland(WaylandDisplayHandle::new(
                NonNull::new(self.display).unwrap(),
            )))
        })
    }
}

impl HasWindowHandle for RawWaylandWindow {
    fn window_handle(&self) -> Result<WindowHandle<'_>, HandleError> {
        Ok(unsafe {
            WindowHandle::borrow_raw(RawWindowHandle::Wayland(WaylandWindowHandle::new(
                std::ptr::NonNull::new_unchecked(self.surface),
            )))
        })
    }
}

/// A raw window handle for AppKit.
///
/// This variant is likely to be used on macOS, although Mac Catalyst
/// $arch-apple-ios-macabi targets.
#[repr(C)]
#[derive(Clone, Copy)]
struct RawAppkitWindow {
    /// A pointer to an NSView object.
    window: *mut c_void,
    width: u32,
    height: u32,
}

unsafe impl Send for RawAppkitWindow {}
unsafe impl Sync for RawAppkitWindow {}

impl GetSize for RawAppkitWindow {
    fn size(&self) -> Size {
        Size {
            width: self.width,
            height: self.height,
        }
    }
}

impl HasDisplayHandle for RawAppkitWindow {
    fn display_handle(&self) -> Result<DisplayHandle<'_>, HandleError> {
        Ok(DisplayHandle::appkit())
    }
}

impl HasWindowHandle for RawAppkitWindow {
    fn window_handle(&self) -> Result<WindowHandle<'_>, HandleError> {
        Ok(unsafe {
            WindowHandle::borrow_raw(RawWindowHandle::AppKit(AppKitWindowHandle::new(
                std::ptr::NonNull::new_unchecked(self.window),
            )))
        })
    }
}

#[repr(C)]
#[derive(Clone, Copy)]
union RawWindowValue {
    win32: RawWin32Window,
    xlib: RawXlibWindow,
    wayland: RawWaylandWindow,
    appkit: RawAppkitWindow,
}

#[repr(C)]
#[allow(unused)]
#[derive(Clone, Copy, PartialEq, Eq)]
enum RawWindowType {
    Win32,
    Xlib,
    Wayland,
    Appkit,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct RawWindowOptions {
    kind: RawWindowType,
    value: RawWindowValue,
}

impl GetSize for RawWindowOptions {
    #[rustfmt::skip]
    fn size(&self) -> Size {
        type T = RawWindowType;
        type V = RawWindowValue;

        unsafe {
            match self {
                Self { kind: T::Win32, value: V { win32 } } => win32.size(),
                Self { kind: T::Xlib, value: V { xlib } } => xlib.size(),
                Self { kind: T::Wayland, value: V { wayland } } => wayland.size(),
                Self { kind: T::Appkit, value: V { appkit } } => appkit.size(),
            }
        }
    }
}

/// Objects that implement this trait should be able to return a DisplayHandle
/// for the display that they are associated with. This handle should last for
/// the lifetime of the object, and should return an error if the application is
/// inactive.
///
/// Objects that implement this trait should be able to return a WindowHandle
/// for the window that they are associated with. This handle should last for
/// the lifetime of the object, and should return an error if the application is
/// inactive.
impl Into<SurfaceTarget<'static>> for RawWindowOptions {
    #[rustfmt::skip]
    fn into(self) -> SurfaceTarget<'static> {
        type T = RawWindowType;
        type V = RawWindowValue;

        SurfaceTarget::Window(unsafe {
            match self {
                Self { kind: T::Win32, value: V { win32 } } => Box::new(win32),
                Self { kind: T::Xlib, value: V { xlib } } => Box::new(xlib),
                Self { kind: T::Wayland, value: V { wayland } } => Box::new(wayland),
                Self { kind: T::Appkit, value: V { appkit } } => Box::new(appkit),
            }
        })
    }
}

/// Configuration of the audio and video streaming player.
#[repr(C)]
#[allow(unused)]
#[derive(Clone, Copy, PartialEq, Eq)]
enum RawAVFrameStreamPlayerType {
    /// Both audio and video will play.
    All,
    /// Play video only.
    OnlyVideo,
    /// Play audio only.
    OnlyAudio,
}

/// Back-end implementation of graphics.
#[repr(C)]
#[allow(unused)]
#[derive(Clone, Copy, PartialEq, Eq)]
enum RawVideoRenderBackend {
    /// Backend implemented using D3D11, which is supported on an older device
    /// and platform and has better performance performance and memory
    /// footprint, but only on windows.
    Direct3D11,
    /// Cross-platform graphics backends implemented using WebGPUs are supported
    /// on a number of common platforms or devices.
    WebGPU,
}

impl Into<VideoRenderBackend> for RawVideoRenderBackend {
    fn into(self) -> VideoRenderBackend {
        match self {
            Self::Direct3D11 => VideoRenderBackend::Direct3D11,
            Self::WebGPU => VideoRenderBackend::WebGPU,
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy)]
struct RawVideoRenderOptions {
    window: RawWindowOptions,
    backend: RawVideoRenderBackend,
}

impl Into<VideoRenderOptions<RawWindowOptions>> for RawVideoRenderOptions {
    fn into(self) -> VideoRenderOptions<RawWindowOptions> {
        VideoRenderOptions {
            backend: self.backend.into(),
            size: self.window.size(),
            target: self.window,
        }
    }
}

#[repr(C)]
union RawAVFrameStreamPlayerValue {
    all: RawVideoRenderOptions,
    only_video: RawVideoRenderOptions,
    only_audio: (),
}

#[repr(C)]
struct RawAVFrameStreamPlayerOptions {
    kind: RawAVFrameStreamPlayerType,
    value: RawAVFrameStreamPlayerValue,
}

impl Into<AVFrameStreamPlayerOptions<RawWindowOptions>> for RawAVFrameStreamPlayerOptions {
    #[rustfmt::skip]
    fn into(self) -> AVFrameStreamPlayerOptions<RawWindowOptions> {
        type U = AVFrameStreamPlayerOptions<RawWindowOptions>;
        type T = RawAVFrameStreamPlayerType;
        type V = RawAVFrameStreamPlayerValue;

        unsafe {
            match self {
                Self { kind: T::All, value: V { all } } => U::All(all.into()),
                Self { kind: T::OnlyVideo, value: V { only_video } } => U::OnlyVideo(only_video.into()),
                Self { kind: T::OnlyAudio, .. } => U::OnlyAudio,
            }
        }
    }
}

pub(crate) type Player = AVFrameStreamPlayer<'static, Callback>;

pub(crate) struct Callback {
    func: Option<extern "C" fn(ctx: *const c_void)>,
    ctx: *const c_void,
}

unsafe impl Sync for Callback {}
unsafe impl Send for Callback {}

impl AVFrameObserver for Callback {
    fn close(&self) {
        if let Some(func) = self.func {
            func(self.ctx);
        }
    }
}

/// Creates the configuration of the player and the callback function is the
/// callback when the stream is closed.
#[repr(C)]
pub(crate) struct RawPlayerOptions {
    options: RawAVFrameStreamPlayerOptions,
    callback: Option<extern "C" fn(ctx: *const c_void)>,
    ctx: *const c_void,
}

impl RawPlayerOptions {
    pub(crate) fn create_player(self) -> Result<Player> {
        Ok(AVFrameStreamPlayer::new(
            self.options.into(),
            Callback {
                func: self.callback,
                ctx: self.ctx,
            },
        )?)
    }
}
