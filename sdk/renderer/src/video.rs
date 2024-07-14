use anyhow::{anyhow, Result};
use common::frame::VideoFrame;
use wgpu::{
    rwh::{
        DisplayHandle, HandleError, HasDisplayHandle, HasWindowHandle, RawWindowHandle,
        Win32WindowHandle, WindowHandle as RWindowHandle,
    },
    Adapter, CommandEncoderDescriptor, CompositeAlphaMode, Device, DeviceDescriptor, Features,
    Instance, Limits, PresentMode, Queue, RequestAdapterOptions, Surface, SurfaceConfiguration,
    TextureFormat, TextureUsages,
};

#[derive(Debug, Clone, Copy)]
pub struct Size {
    pub width: u32,
    pub height: u32,
}

pub enum WindowHandle {
    Win32(Win32WindowHandle),
}

impl HasWindowHandle for WindowHandle {
    fn window_handle(&self) -> Result<RWindowHandle, HandleError> {
        Ok(unsafe {
            RWindowHandle::borrow_raw(match self {
                Self::Win32(handle) => RawWindowHandle::Win32(handle.clone()),
            })
        })
    }
}

impl HasDisplayHandle for WindowHandle {
    fn display_handle(&self) -> Result<DisplayHandle, HandleError> {
        Ok(match self {
            Self::Win32(_) => DisplayHandle::windows(),
        })
    }
}

pub struct VideoRender<'a> {
    instance: Instance,
    surface: Surface<'a>,
    adapter: Adapter,
    device: Device,
    queue: Queue,
}

impl<'a> VideoRender<'a> {
    pub fn new(size: Size, handle: &'a WindowHandle) -> Result<Self> {
        let instance = Instance::default();
        let surface = instance.create_surface(handle)?;

        let adapter = pollster::block_on(instance.request_adapter(&RequestAdapterOptions {
            compatible_surface: Some(&surface),
            ..Default::default()
        }))
        .ok_or_else(|| anyhow!("not found a gpu adapter"))?;

        let (device, queue) = pollster::block_on(adapter.request_device(
            &DeviceDescriptor {
                label: None,
                required_features: Features::empty(),
                required_limits: Limits::downlevel_defaults(),
            },
            None,
        ))?;

        surface.configure(
            &device,
            &SurfaceConfiguration {
                usage: TextureUsages::all(),
                format: TextureFormat::Bgra8UnormSrgb,
                width: size.width,
                height: size.height,
                present_mode: PresentMode::AutoVsync,
                desired_maximum_frame_latency: 2,
                alpha_mode: CompositeAlphaMode::Opaque,
                view_formats: vec![],
            },
        );

        Ok(Self {
            instance,
            surface,
            adapter,
            device,
            queue,
        })
    }

    pub fn send(&self, frame: &VideoFrame) -> Result<()> {
        let output = self.surface.get_current_texture()?;
        let mut encoder = self
            .device
            .create_command_encoder(&CommandEncoderDescriptor { label: None });

        {}

        self.queue.submit([encoder.finish()]);

        output.present();
        todo!()
    }

    pub fn resize(&self, size: Size) -> Result<()> {
        todo!()
    }
}
