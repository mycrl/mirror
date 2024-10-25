use crate::{Events, MirrorBackend};

use std::sync::Arc;

use anyhow::anyhow;
use mirror::{AVFrameObserver, AVFrameSink, AVFrameStream, AudioFrame, Renderer, VideoFrame};
use napi::{
    threadsafe_function::{ThreadsafeFunction, ThreadsafeFunctionCallMode},
    JsUnknown,
};

use napi_derive::napi;
use winit::{
    application::ApplicationHandler,
    dpi::{PhysicalPosition, PhysicalSize},
    event::WindowEvent,
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    window::{Fullscreen, Window as WinitWindow, WindowId},
};

pub type Callback = ThreadsafeFunction<Events, JsUnknown, Events, false>;

#[napi(object)]
#[derive(Debug, Default, Clone, Copy)]
pub struct Position {
    pub x: i32,
    pub y: i32,
}

impl Into<winit::dpi::Position> for Position {
    fn into(self) -> winit::dpi::Position {
        winit::dpi::Position::Physical(PhysicalPosition::new(self.x, self.y))
    }
}

#[napi(object)]
#[derive(Debug, Default, Clone, Copy)]
pub struct Size {
    pub width: u32,
    pub height: u32,
}

impl Into<winit::dpi::Size> for Size {
    fn into(self) -> winit::dpi::Size {
        winit::dpi::Size::Physical(PhysicalSize::new(self.width, self.height))
    }
}

impl Into<mirror::Size> for Size {
    fn into(self) -> mirror::Size {
        mirror::Size {
            width: self.width,
            height: self.height,
        }
    }
}

#[napi(object)]
#[derive(Debug, Default, Clone)]
pub struct WindowDescriptor {
    pub backend: MirrorBackend,
    pub title: String,
    pub size: Size,
    pub position: Position,
    pub resizable: bool,
    pub maximized: bool,
    pub visible: bool,
    pub transparent: bool,
    pub blur: bool,
    pub decorations: bool,
    pub active: bool,
    pub fullscreen: bool,
}

#[napi]
#[derive(Clone)]
pub struct Window {
    options: WindowDescriptor,
    window: Option<Arc<WinitWindow>>,
}

#[napi]
impl Window {
    #[napi(constructor)]
    pub fn new(options: WindowDescriptor) -> napi::Result<Self> {
        Ok(Self {
            window: None,
            options,
        })
    }

    #[napi]
    pub fn set_visible(&self, visible: bool) {
        if let Some(window) = self.window.as_ref() {
            window.set_visible(visible);
        }
    }

    #[napi]
    pub fn set_fullscreen(&self, fullscreen: bool) {
        if let Some(window) = self.window.as_ref() {
            window.set_fullscreen(if fullscreen {
                Some(Fullscreen::Borderless(None))
            } else {
                None
            });
        }
    }

    #[napi]
    pub fn start(&mut self) -> napi::Result<()> {
        let mut func = || {
            let event_loop = EventLoop::new()?;
            event_loop.set_control_flow(ControlFlow::Wait);
            event_loop.run_app(self)?;

            Ok::<(), anyhow::Error>(())
        };

        func().map_err(|e| napi::Error::from_reason(e.to_string()))
    }

    pub(crate) fn create_renderer(&self) -> Result<Renderer, anyhow::Error> {
        if let Some(window) = self.window.as_ref() {
            Ok(Renderer::new(
                self.options.backend.into(),
                window,
                self.options.size.into(),
            )?)
        } else {
            Err(anyhow!("window is not created"))
        }
    }
}

impl ApplicationHandler for Window {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let mut func = move || {
            let mut attr = WinitWindow::default_attributes();
            attr.title = self.options.title.clone();
            attr.inner_size = Some(self.options.size.into());
            attr.position = Some(self.options.position.into());
            attr.resizable = self.options.resizable;
            attr.maximized = self.options.maximized;
            attr.visible = self.options.visible;
            attr.transparent = self.options.transparent;
            attr.blur = self.options.blur;
            attr.decorations = self.options.decorations;
            attr.active = self.options.active;
            attr.fullscreen = if self.options.fullscreen {
                Some(Fullscreen::Borderless(None))
            } else {
                None
            };

            let window = Arc::new(event_loop.create_window(attr)?);
            self.window.replace(window.clone());

            Ok::<(), anyhow::Error>(())
        };

        if func().is_err() {
            event_loop.exit();
        }
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        event: WindowEvent,
    ) {
        match event {
            WindowEvent::CloseRequested => {
                event_loop.exit();
            }
            _ => (),
        }
    }
}

/// This is an empty window implementation that doesn't render any audio or
/// video and is only used to handle close events.
pub struct EmptyWindow(pub Callback);

impl AVFrameStream for EmptyWindow {}
impl AVFrameSink for EmptyWindow {}

impl AVFrameObserver for EmptyWindow {
    fn close(&self) {
        self.0
            .call(Events::Closed, ThreadsafeFunctionCallMode::NonBlocking);
    }
}

/// Renders video frames and audio/video frames to the native window.
pub struct NativeWindow(pub Callback);

impl AVFrameStream for NativeWindow {}

impl AVFrameSink for NativeWindow {
    fn audio(&self, frame: &AudioFrame) -> bool {
        if let Some(renderer) = self.renderer.as_ref() {
            renderer.audio(frame)
        } else {
            true
        }
    }

    fn video(&self, frame: &VideoFrame) -> bool {
        if let Some(renderer) = self.renderer.as_ref() {
            renderer.video(frame)
        } else {
            true
        }
    }
}

impl AVFrameObserver for NativeWindow {
    fn initialized(&self) {
        self.0
            .call(Events::Initialized, ThreadsafeFunctionCallMode::NonBlocking);
    }

    fn close(&self) {
        self.0
            .call(Events::Closed, ThreadsafeFunctionCallMode::NonBlocking);
    }
}
