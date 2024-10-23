use std::{ffi::c_void, ptr::NonNull, sync::Arc};

use mirror::{
    raw_window_handle::{
        AppKitWindowHandle, DisplayHandle, HandleError, HasDisplayHandle, HasWindowHandle,
        RawDisplayHandle, RawWindowHandle, Win32WindowHandle, WindowHandle, XlibDisplayHandle,
        XlibWindowHandle,
    },
    AVFrameSink, AVFrameStream, AudioFrame, Close, Renderer, Size, VideoFrame,
};

use napi::{
    threadsafe_function::{ThreadsafeFunction, ThreadsafeFunctionCallMode},
    JsBigInt, JsUnknown,
};

use napi_derive::napi;

#[napi(object)]
#[derive(Clone)]
pub struct WindowsNativeWindowHandle {
    /// A handle to a window.
    ///
    /// This type is declared in WinDef.h as follows:
    ///
    /// typedef HANDLE HWND;
    pub hwnd: JsBigInt,
    pub width: u32,
    pub height: u32,
}

#[napi(object)]
#[derive(Clone)]
pub struct LinuxNativeWindowHandle {
    /// typedef unsigned long int XID;
    ///
    /// typedef XID Window;
    pub window: u32,
    pub display: JsBigInt,
    pub screen: i32,
    pub width: u32,
    pub height: u32,
}

#[napi(object)]
#[derive(Clone)]
pub struct MacosNativeWindowHandle {
    /// The infrastructure for drawing, printing, and handling events in an app.
    ///
    /// AppKit handles most of your app’s NSView management. Unless you’re
    /// implementing a concrete subclass of NSView or working intimately with
    /// the content of the view hierarchy at runtime, you don’t need to know
    /// much about this class’s interface. For any view, there are many methods
    /// that you can use as-is. The following methods are commonly used.
    pub ns_view: JsBigInt,
    pub width: u32,
    pub height: u32,
}

/// A window handle for a particular windowing system.
#[derive(Clone)]
pub enum NativeWindowHandle {
    Windows(WindowsNativeWindowHandle),
    Linux(LinuxNativeWindowHandle),
    Macos(MacosNativeWindowHandle),
}

unsafe impl Send for NativeWindowHandle {}
unsafe impl Sync for NativeWindowHandle {}

impl NativeWindowHandle {
    pub fn size(&self) -> Size {
        match self {
            Self::Windows(WindowsNativeWindowHandle { width, height, .. })
            | Self::Linux(LinuxNativeWindowHandle { width, height, .. })
            | Self::Macos(MacosNativeWindowHandle { width, height, .. }) => Size {
                width: *width,
                height: *height,
            },
        }
    }
}

impl HasDisplayHandle for NativeWindowHandle {
    fn display_handle(&self) -> Result<DisplayHandle<'_>, HandleError> {
        Ok(match self {
            Self::Macos(_) => DisplayHandle::appkit(),
            Self::Windows(_) => DisplayHandle::windows(),
            // This variant is likely to show up anywhere someone manages to get X11 working
            // that Xlib can be built for, which is to say, most (but not all) Unix systems.
            Self::Linux(LinuxNativeWindowHandle {
                display, screen, ..
            }) => unsafe {
                DisplayHandle::borrow_raw(RawDisplayHandle::Xlib(XlibDisplayHandle::new(
                    NonNull::new(display.get_i64().unwrap().0 as *mut c_void),
                    *screen,
                )))
            },
        })
    }
}

impl HasWindowHandle for NativeWindowHandle {
    fn window_handle(&self) -> Result<WindowHandle<'_>, HandleError> {
        Ok(match self {
            // This variant is used on Windows systems.
            Self::Windows(WindowsNativeWindowHandle { hwnd, .. }) => unsafe {
                WindowHandle::borrow_raw(RawWindowHandle::Win32(Win32WindowHandle::new(
                    std::num::NonZeroIsize::new(hwnd.get_i64().unwrap().0 as isize).unwrap(),
                )))
            },
            // This variant is likely to show up anywhere someone manages to get X11
            // working that Xlib can be built for, which is to say, most (but not all)
            // Unix systems.
            Self::Linux(LinuxNativeWindowHandle { window, .. }) => unsafe {
                WindowHandle::borrow_raw(RawWindowHandle::Xlib(XlibWindowHandle::new(*window)))
            },
            // This variant is likely to be used on macOS, although Mac Catalyst
            // ($arch-apple-ios-macabi targets, which can notably use UIKit or AppKit) can also
            // use it despite being target_os = "ios".
            Self::Macos(MacosNativeWindowHandle { ns_view, .. }) => unsafe {
                WindowHandle::borrow_raw(RawWindowHandle::AppKit(AppKitWindowHandle::new(
                    std::ptr::NonNull::new_unchecked(ns_view.get_i64().unwrap().0 as *mut c_void),
                )))
            },
        })
    }
}

/// Renders video frames and audio/video frames to the native window.
pub struct Window {
    pub callback: ThreadsafeFunction<(), JsUnknown, (), false>,
    pub renderer: Arc<Renderer<'static>>,
}

impl AVFrameStream for Window {}

impl AVFrameSink for Window {
    fn video(&self, frame: &VideoFrame) -> bool {
        self.renderer.video(frame)
    }

    fn audio(&self, frame: &AudioFrame) -> bool {
        self.renderer.audio(frame)
    }
}

impl Close for Window {
    fn close(&self) {
        self.callback
            .call((), ThreadsafeFunctionCallMode::NonBlocking);
    }
}

/// This is an empty window implementation that doesn't render any audio or
/// video and is only used to handle close events.
pub struct EmptyWindow(pub ThreadsafeFunction<(), JsUnknown, (), false>);

impl AVFrameStream for EmptyWindow {}
impl AVFrameSink for EmptyWindow {}

impl Close for EmptyWindow {
    fn close(&self) {
        self.0.call((), ThreadsafeFunctionCallMode::NonBlocking);
    }
}
