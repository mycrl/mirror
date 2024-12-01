use std::{
    ffi::{c_ulong, c_void},
    ptr::NonNull,
    sync::Arc,
};

use hylarana::{
    raw_window_handle::{
        AppKitWindowHandle, DisplayHandle, HandleError, HasDisplayHandle, HasWindowHandle,
        RawDisplayHandle, RawWindowHandle, Win32WindowHandle, WindowHandle, XlibDisplayHandle,
        XlibWindowHandle,
    },
    AVFrameObserver, AVFrameSink, AVFrameStream, AudioFrame, Renderer, Size, VideoFrame,
};

use napi::{
    threadsafe_function::{ThreadsafeFunction, ThreadsafeFunctionCallMode},
    JsBigInt, JsUnknown,
};

use napi_derive::napi;

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

impl AVFrameObserver for Window {
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

impl AVFrameObserver for EmptyWindow {
    fn close(&self) {
        self.0.call((), ThreadsafeFunctionCallMode::NonBlocking);
    }
}
