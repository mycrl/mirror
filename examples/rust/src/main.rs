use std::{net::SocketAddr, sync::Arc};

use anyhow::Result;
use clap::{
    builder::{PossibleValuesParser, TypedValueParser},
    Parser,
};

use mirror::{
    shutdown, startup, AVFrameSink, AVFrameStream, AudioDescriptor, AudioFrame, Capture, Close,
    GraphicsBackend, Mirror, Receiver, ReceiverDescriptor, Renderer, Sender, SenderDescriptor,
    SourceType, TransportDescriptor, VideoDecoderType, VideoDescriptor, VideoEncoderType,
    VideoFrame,
};
use mirror_common::Size;

use winit::{
    application::ApplicationHandler,
    dpi::PhysicalSize,
    event::{ElementState, WindowEvent},
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop, EventLoopProxy},
    keyboard::{KeyCode, PhysicalKey},
    window::{Window, WindowId},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StreamKind {
    Sender,
    Receiver,
}

struct Canvas {
    kind: StreamKind,
    renderer: Arc<Renderer<'static>>,
    event_proxy: EventLoopProxy<AppEvent>,
}

impl AVFrameStream for Canvas {}

impl AVFrameSink for Canvas {
    fn audio(&self, frame: &AudioFrame) -> bool {
        if self.kind == StreamKind::Receiver {
            return self.renderer.audio(frame);
        }

        true
    }

    fn video(&self, frame: &VideoFrame) -> bool {
        self.renderer.video(frame)
    }
}

impl Close for Canvas {
    fn close(&self) {
        let _ = self.event_proxy.send_event(match self.kind {
            StreamKind::Receiver => AppEvent::CloseReceiver,
            StreamKind::Sender => AppEvent::CloseSender,
        });
    }
}

#[derive(Debug, Clone, Copy)]
enum AppEvent {
    CloseSender,
    CloseReceiver,
}

struct App {
    cli: Cli,
    event_proxy: EventLoopProxy<AppEvent>,
    window: Option<Arc<Window>>,
    renderer: Option<Arc<Renderer<'static>>>,
    mirror: Option<Mirror>,
    sender: Option<Sender>,
    receiver: Option<Receiver>,
}

impl App {
    fn new(cli: Cli, event_proxy: EventLoopProxy<AppEvent>) -> Self {
        Self {
            cli,
            event_proxy,
            window: None,
            renderer: None,
            mirror: None,
            sender: None,
            receiver: None,
        }
    }

    fn create_canvas(&self, kind: StreamKind) -> Option<Canvas> {
        Some(Canvas {
            kind,
            renderer: self.renderer.clone()?,
            event_proxy: self.event_proxy.clone(),
        })
    }

    fn create_window(&mut self, event_loop: &ActiveEventLoop) -> Result<()> {
        let mut attr = Window::default_attributes();
        attr.inner_size = Some(winit::dpi::Size::Physical(PhysicalSize::new(
            self.cli.width,
            self.cli.height,
        )));

        let window = Arc::new(event_loop.create_window(attr)?);

        self.renderer.replace(Arc::new(Renderer::new(
            GraphicsBackend::WebGPU,
            window.clone(),
            Size {
                width: self.cli.width,
                height: self.cli.height,
            },
        )?));

        self.window.replace(window);
        self.mirror.replace(Mirror::new(TransportDescriptor {
            multicast: "239.0.0.1".parse()?,
            server: self.cli.server,
            mtu: 1500,
        })?);

        startup()?;
        Ok(())
    }

    fn create_sender(&mut self) -> Result<()> {
        let mut options = SenderDescriptor::default();

        if let Some(source) = Capture::get_sources(SourceType::Screen)?.get(0) {
            options.video = Some((
                source.clone(),
                VideoDescriptor {
                    codec: self.cli.encoder.unwrap(),
                    frame_rate: self.cli.fps,
                    width: self.cli.width,
                    height: self.cli.height,
                    bit_rate: 500 * 1024 * 8,
                    key_frame_interval: 21,
                },
            ));
        }

        if let Some(source) = Capture::get_sources(SourceType::Audio)?.get(0) {
            options.audio = Some((
                source.clone(),
                AudioDescriptor {
                    sample_rate: 48000,
                    bit_rate: 64000,
                },
            ));
        }

        if let (Some(mirror), Some(canvas)) =
            (self.mirror.as_ref(), self.create_canvas(StreamKind::Sender))
        {
            self.sender
                .replace(mirror.create_sender(self.cli.id, options, canvas)?);
        }

        Ok(())
    }

    fn create_receiver(&mut self) -> Result<()> {
        if let (Some(mirror), Some(canvas)) = (
            self.mirror.as_ref(),
            self.create_canvas(StreamKind::Receiver),
        ) {
            self.receiver.replace(mirror.create_receiver(
                self.cli.id,
                ReceiverDescriptor {
                    video: self.cli.decoder.unwrap(),
                },
                canvas,
            )?);
        }

        Ok(())
    }
}

impl ApplicationHandler<AppEvent> for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        self.create_window(event_loop).unwrap();
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        event: WindowEvent,
    ) {
        match event {
            // The user closes the window, and we close the sender and receiver, in that order, and
            // release the renderer and mirror instances, and finally stop the message loop.
            WindowEvent::CloseRequested => {
                drop(self.receiver.take());
                drop(self.sender.take());
                drop(self.renderer.take());
                drop(self.mirror.take());

                event_loop.exit();
            }
            WindowEvent::KeyboardInput { event, .. } => {
                if !event.repeat && event.state == ElementState::Released {
                    if let PhysicalKey::Code(key) = event.physical_key {
                        match key {
                            // When the S key is pressed, the sender is created, but check to see if
                            // the sender has already been created between sender creation to avoid
                            // duplicate creation.
                            //
                            // The receiving end is the same.
                            KeyCode::KeyS => {
                                if self.sender.is_none() {
                                    if let Err(e) = self.create_sender() {
                                        log::error!("{:?}", e);
                                    }
                                }
                            }
                            KeyCode::KeyR => {
                                if self.receiver.is_none() {
                                    if let Err(e) = self.create_receiver() {
                                        log::error!("{:?}", e);
                                    }
                                }
                            }
                            // When the S key is pressed, either the transmitter or the receiver
                            // needs to be turned off. No distinction is made here; both the
                            // transmitter and the receiver are turned off.
                            KeyCode::KeyK => {
                                let _ = self.event_proxy.send_event(AppEvent::CloseSender);
                                let _ = self.event_proxy.send_event(AppEvent::CloseReceiver);
                            }
                            _ => (),
                        }
                    }
                }
            }
            _ => (),
        }
    }

    fn user_event(&mut self, _event_loop: &ActiveEventLoop, event: AppEvent) {
        // Handle events that close the sender or close the receiver. When a close event
        // is received, we need to drop the corresponding sender or receiver.
        match event {
            AppEvent::CloseSender => drop(self.sender.take()),
            AppEvent::CloseReceiver => drop(self.receiver.take()),
        }
    }
}

#[derive(Parser, Debug)]
#[command(
    about = env!("CARGO_PKG_DESCRIPTION"),
    version = env!("CARGO_PKG_VERSION"),
    author = env!("CARGO_PKG_AUTHORS"),
)]
struct Cli {
    /// The address to which the mirror service is bound, indicating how to
    /// connect to the mirror service.
    #[arg(long)]
    server: SocketAddr,
    #[arg(long, default_value_t = 1280)]
    width: u32,
    #[arg(long, default_value_t = 720)]
    height: u32,
    #[arg(long, default_value_t = 24)]
    fps: u8,
    /// Each sender and receiver need to be bound to a channel, and the receiver
    /// can only receive the cast screen within the channel.
    #[arg(long, default_value_t = 0)]
    id: u32,
    #[arg(
        long,
        value_parser = PossibleValuesParser::new(["libx264", "h264_qsv", "h264_videotoolbox"])
            .map(|s| s.parse::<VideoEncoderType>()),
    )]
    encoder: Option<VideoEncoderType>,
    #[arg(
        long,
        value_parser = PossibleValuesParser::new(["h264", "d3d11va", "h264_qsv", "h264_videotoolbox"])
            .map(|s| s.parse::<VideoDecoderType>()),
    )]
    decoder: Option<VideoDecoderType>,
}

fn main() -> Result<()> {
    simple_logger::init_with_level(log::Level::Info)?;

    let mut cli = Cli::parse();

    // Use different default codecs on different platforms, it is better to use
    // hardware codecs by default compared to software codecs.
    cli.encoder.replace(if cfg!(target_os = "macos") {
        VideoEncoderType::VideoToolBox
    } else if cfg!(target_os = "windows") {
        VideoEncoderType::Qsv
    } else {
        VideoEncoderType::X264
    });

    cli.decoder.replace(if cfg!(target_os = "macos") {
        VideoDecoderType::VideoToolBox
    } else if cfg!(target_os = "windows") {
        VideoDecoderType::D3D11
    } else {
        VideoDecoderType::H264
    });

    // Creates a message loop, which is used to create the main window.
    let event_loop = EventLoop::<AppEvent>::with_user_event().build()?;
    event_loop.set_control_flow(ControlFlow::Wait);

    let event_proxy = event_loop.create_proxy();
    event_loop.run_app(&mut App::new(cli, event_proxy))?;

    //When exiting the application, the environment of mirror should be cleaned up.
    shutdown()?;
    Ok(())
}
