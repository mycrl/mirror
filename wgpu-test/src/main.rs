use std::{any::Any, sync::Arc};

use anyhow::{anyhow, Result};
use wgpu::{
    include_wgsl, Adapter, Backends, BindGroupDescriptor, BindGroupEntry, BindGroupLayoutEntry, BindingResource, BindingType, BlendState, ColorTargetState, ColorWrites, CommandEncoderDescriptor, CompositeAlphaMode, Device, DeviceDescriptor, FragmentState, Instance, InstanceDescriptor, MultisampleState, PipelineLayoutDescriptor, PresentMode, PrimitiveState, PrimitiveTopology, Queue, RenderPassColorAttachment, RenderPassDescriptor, RenderPipeline, RenderPipelineDescriptor, RequestAdapterOptions, Sampler, SamplerDescriptor, ShaderStages, Surface, SurfaceConfiguration, TextureFormat, TextureUsages, VertexState
};
use winit::{
    application::ApplicationHandler,
    event::WindowEvent,
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    window::{Window, WindowId},
};

struct Renderer {
    instence: Instance,
    surface: Surface<'static>,
    adapter: Adapter,
    device: Device,
    queue: Queue,
    config: SurfaceConfiguration,
    sampler: Sampler,
    pipeline: RenderPipeline,
}

impl Renderer {
    fn create(window: Arc<Window>) -> Result<Self> {
        let size = window.inner_size();
        let config = SurfaceConfiguration {
            usage: TextureUsages::RENDER_ATTACHMENT,
            format: TextureFormat::Rgba8Unorm,
            width: size.width,
            height: size.height,
            present_mode: PresentMode::Fifo,
            alpha_mode: CompositeAlphaMode::Auto,
            desired_maximum_frame_latency: 2,
            view_formats: Vec::new(),
        };

        let instence = Instance::new(InstanceDescriptor {
            backends: Backends::all(),
            ..Default::default()
        });

        let surface = instence.create_surface(window)?;
        let adapter =
            pollster::block_on(instence.request_adapter(&RequestAdapterOptions::default()))
                .ok_or_else(|| anyhow!("not found a adapter"))?;

        let (device, queue) =
            pollster::block_on(adapter.request_device(&DeviceDescriptor::default(), None))?;
        surface.configure(&device, &config);

        let sampler = device.create_sampler(&SamplerDescriptor::default());
        let pipeline = device.create_render_pipeline(&RenderPipelineDescriptor {
            label: Some("RendererPipeline"),
            layout: Some(&device.create_pipeline_layout(&PipelineLayoutDescriptor::default())),
            vertex: VertexState {
                module: &device.create_shader_module(include_wgsl!("./shader.wgsl")),
                compilation_options: Default::default(),
                entry_point: "vert_main",
                buffers: &[],
            },
            fragment: Some(FragmentState {
                module: &device.create_shader_module(include_wgsl!("./fragment.wgsl")),
                compilation_options: Default::default(),
                entry_point: "frag_main",
                targets: &[Some(ColorTargetState {
                    format: config.format,
                    blend: Some(BlendState::REPLACE),
                    write_mask: ColorWrites::ALL,
                })],
            }),
            primitive: PrimitiveState {
                topology: PrimitiveTopology::TriangleList,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            multiview: None,
            cache: None,
        });

        Ok(Self {
            instence,
            surface,
            adapter,
            device,
            queue,
            config,
            sampler,
            pipeline,
        })
    }

    fn render(&mut self) -> Result<()> {
        let surface_texture = self.surface.get_current_texture()?;
        let view = surface_texture.texture.create_view(&wgpu::TextureViewDescriptor::default());

        let bind_group = self.device.create_bind_group(&BindGroupDescriptor {
            label: None,
            layout: &self.pipeline.get_bind_group_layout(0),
            entries: &[
                BindGroupEntry {
                    binding: 0,
                    resource: BindingResource::Sampler(&self.sampler),
                },
                BindGroupEntry {
                    binding: 1,
                    resource: BindingResource::TextureView(&view),
                }
            ]
        });

        let encoder = self.device.create_command_encoder(&CommandEncoderDescriptor::default());
        encoder.begin_render_pass(&RenderPassDescriptor {
            color_attachments: &[
                Some(RenderPassColorAttachment {
                    view: &view,
                    
                })
            ],
            ..Default::default()
        });

        Ok(())
    }
}

#[derive(Default)]
struct App {
    window: Option<Arc<Window>>,
    renderer: Option<Renderer>,
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let window = Arc::new(
            event_loop
                .create_window(Window::default_attributes())
                .unwrap(),
        );
        self.renderer = Some(Renderer::create(window.clone()).unwrap());
        self.window = Some(window);
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => {
                event_loop.exit();
            }
            WindowEvent::RedrawRequested => {
                if let Some(window) = self.window.as_ref() {
                    window.request_redraw();
                }
            }
            _ => (),
        }
    }
}

fn main() -> Result<()> {
    let event_loop = EventLoop::new()?;
    event_loop.set_control_flow(ControlFlow::Poll);

    let mut app = App::default();
    event_loop.run_app(&mut app)?;
    Ok(())
}
