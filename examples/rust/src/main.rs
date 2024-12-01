use std::{
    collections::HashMap,
    net::{IpAddr, SocketAddr},
    sync::Arc,
};

use anyhow::{anyhow, Result};
use clap::Parser;
use hylarana::{
    shutdown, startup, AVFrameObserver, AVFrameStreamPlayer, AVFrameStreamPlayerOptions,
    AudioOptions, Capture, DiscoveryService, Hylarana, HylaranaReceiver,
    HylaranaReceiverCodecOptions, HylaranaReceiverOptions, HylaranaSender,
    HylaranaSenderMediaOptions, HylaranaSenderOptions, HylaranaSenderTrackOptions, Size,
    SourceType, TransportOptions, TransportStrategy, VideoDecoderType, VideoEncoderType,
    VideoOptions, VideoRenderBackend, VideoRenderOptions,
};

use parking_lot::Mutex;
use winit::{
    application::ApplicationHandler,
    dpi::PhysicalSize,
    event::{ElementState, WindowEvent},
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    keyboard::{KeyCode, PhysicalKey},
    window::{Window, WindowId},
};

type Properties = HashMap<String, String>;

struct StreamInfo {
    id: String,
    strategy: TransportStrategy,
}

impl Into<Properties> for StreamInfo {
    fn into(self) -> Properties {
        let mut map = HashMap::with_capacity(3);
        map.insert("id".to_string(), self.id);
        map.insert(
            "strategy".to_string(),
            match self.strategy {
                TransportStrategy::Direct(_) => 0,
                TransportStrategy::Relay(_) => 1,
                TransportStrategy::Multicast(_) => 2,
            }
            .to_string(),
        );

        match self.strategy {
            TransportStrategy::Direct(addr)
            | TransportStrategy::Relay(addr)
            | TransportStrategy::Multicast(addr) => {
                map.insert("address".to_string(), addr.to_string());
            }
        }

        map
    }
}

impl TryFrom<Properties> for StreamInfo {
    type Error = anyhow::Error;

    fn try_from(value: Properties) -> Result<Self, Self::Error> {
        (|| {
            let address: SocketAddr = value.get("address")?.parse().ok()?;
            let strategy = match value.get("strategy")?.as_str().parse::<i32>().ok()? {
                0 => TransportStrategy::Direct(address),
                1 => TransportStrategy::Relay(address),
                2 => TransportStrategy::Multicast(address),
                _ => return None,
            };

            Some(Self {
                id: value.get("id")?.clone(),
                strategy,
            })
        })()
        .ok_or_else(|| anyhow!("invalid properties"))
    }
}

trait GetSize {
    fn size(&self) -> Size;
}

impl GetSize for Window {
    fn size(&self) -> Size {
        let size = self.inner_size();
        Size {
            width: size.width,
            height: size.height,
        }
    }
}

struct ViewObserver;

impl AVFrameObserver for ViewObserver {
    fn close(&self) {
        println!("view is closed");
    }
}

#[allow(unused)]
struct Sender {
    sender: HylaranaSender<AVFrameStreamPlayer<'static, ViewObserver>>,
    discovery: DiscoveryService,
}

impl Sender {
    fn new(configure: &Configure, window: Arc<Window>) -> Result<Self> {
        // Get the first screen that can be captured.
        let mut video = None;
        if let Some(source) = Capture::get_sources(SourceType::Screen)?.get(0) {
            video = Some(HylaranaSenderTrackOptions {
                options: configure.get_video_options(),
                source: source.clone(),
            });
        }

        // Get the first audio input device that can be captured.
        let mut audio = None;
        if let Some(source) = Capture::get_sources(SourceType::Audio)?.get(0) {
            audio = Some(HylaranaSenderTrackOptions {
                source: source.clone(),
                options: AudioOptions {
                    sample_rate: 48000,
                    bit_rate: 64000,
                },
            });
        }

        let strategy = configure.get_strategy().unwrap();
        let sender = Hylarana::create_sender(
            HylaranaSenderOptions {
                transport: TransportOptions {
                    strategy,
                    mtu: 1500,
                },
                media: HylaranaSenderMediaOptions { video, audio },
            },
            AVFrameStreamPlayer::new(
                AVFrameStreamPlayerOptions::OnlyVideo(VideoRenderOptions {
                    backend: VideoRenderBackend::WebGPU,
                    size: window.size(),
                    target: window,
                }),
                ViewObserver,
            )?,
        )?;

        // Register the current sender's information with the LAN discovery service so
        // that other receivers can know that the sender has been created and can access
        // the sender's information.
        let discovery = DiscoveryService::register::<Properties>(
            3456,
            &StreamInfo {
                id: sender.get_id().to_string(),
                strategy,
            }
            .into(),
        )?;

        Ok(Self { discovery, sender })
    }
}

#[allow(unused)]
struct Receiver {
    receiver: Arc<Mutex<Option<HylaranaReceiver<AVFrameStreamPlayer<'static, ViewObserver>>>>>,
    discovery: DiscoveryService,
}

impl Receiver {
    fn new(configure: &Configure, window: Arc<Window>) -> Result<Self> {
        let video_decoder = configure.decoder;

        let receiver = Arc::new(Mutex::new(None));
        let receiver_ = Arc::downgrade(&receiver);

        // Find published senders through the LAN discovery service.
        let discovery = DiscoveryService::query(move |addrs, properties: Properties| {
            if let Some(receiver) = receiver_.upgrade() {
                // If the sender has already been created, no further sender postings are
                // processed.
                if receiver.lock().is_some() {
                    return;
                }

                let mut properties = StreamInfo::try_from(properties).unwrap();

                // The sender, if using passthrough, will need to replace the ip in the publish
                // address by replacing the ip address with the sender's ip.
                if let TransportStrategy::Direct(addr) = &mut properties.strategy {
                    addr.set_ip(IpAddr::V4(addrs[0]));
                }

                if let Ok(it) = Hylarana::create_receiver(
                    properties.id,
                    HylaranaReceiverOptions {
                        codec: HylaranaReceiverCodecOptions {
                            video: video_decoder,
                        },
                        transport: TransportOptions {
                            strategy: properties.strategy,
                            mtu: 1500,
                        },
                    },
                    AVFrameStreamPlayer::new(
                        AVFrameStreamPlayerOptions::All(VideoRenderOptions {
                            backend: VideoRenderBackend::WebGPU,
                            size: window.size(),
                            target: window.clone(),
                        }),
                        ViewObserver,
                    )
                    .unwrap(),
                ) {
                    receiver.lock().replace(it);
                }
            }
        })?;

        Ok(Self {
            discovery,
            receiver,
        })
    }
}

#[derive(Default)]
struct App {
    window: Option<Arc<Window>>,
    receiver: Option<Receiver>,
    sender: Option<Sender>,
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let configure = Configure::parse();

        (|| {
            let mut attr = Window::default_attributes();
            attr.title = "hylarana example".to_string();
            attr.inner_size = Some(winit::dpi::Size::Physical(PhysicalSize::new(
                configure.width,
                configure.height,
            )));

            self.window
                .replace(Arc::new(event_loop.create_window(attr)?));
            startup()?;

            Ok::<_, anyhow::Error>(())
        })()
        .unwrap()
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        event: WindowEvent,
    ) {
        match event {
            // The user closes the window, and we close the sender and receiver, in that order, and
            // release the renderer and hylarana instances, and finally stop the message loop.
            WindowEvent::CloseRequested => {
                drop(self.sender.take());
                drop(self.receiver.take());

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
                                if let (None, Some(window)) = (&self.sender, &self.window) {
                                    self.sender.replace(
                                        Sender::new(&Configure::parse(), window.clone()).unwrap(),
                                    );
                                }
                            }
                            KeyCode::KeyR => {
                                if let (None, Some(window)) = (&self.receiver, &self.window) {
                                    self.receiver.replace(
                                        Receiver::new(&Configure::parse(), window.clone()).unwrap(),
                                    );
                                }
                            }
                            // When the S key is pressed, either the transmitter or the receiver
                            // needs to be turned off. No distinction is made here; both the
                            // transmitter and the receiver are turned off.
                            KeyCode::KeyK => {
                                drop(self.receiver.take());
                                drop(self.sender.take());
                            }
                            _ => (),
                        }
                    }
                }
            }
            _ => (),
        }
    }
}

#[derive(Parser)]
#[command(
    about = env!("CARGO_PKG_DESCRIPTION"),
    version = env!("CARGO_PKG_VERSION"),
    author = env!("CARGO_PKG_AUTHORS"),
)]
struct Configure {
    /// The address to which the hylarana service is bound, indicating how to
    /// connect to the hylarana service.
    #[arg(long)]
    address: Option<SocketAddr>,
    #[arg(long)]
    strategy: Option<String>,
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
        value_parser = clap::value_parser!(VideoEncoderType),
        default_value_t = Self::DEFAULT_ENCODER,
    )]
    encoder: VideoEncoderType,
    #[arg(
        long,
        value_parser = clap::value_parser!(VideoDecoderType),
        default_value_t = Self::DEFAULT_DECODER,
    )]
    decoder: VideoDecoderType,
}

impl Configure {
    #[cfg(target_os = "macos")]
    const DEFAULT_ENCODER: VideoEncoderType = VideoEncoderType::VideoToolBox;

    #[cfg(target_os = "windows")]
    const DEFAULT_ENCODER: VideoEncoderType = VideoEncoderType::Qsv;

    #[cfg(target_os = "linux")]
    const DEFAULT_ENCODER: VideoEncoderType = VideoEncoderType::X264;

    #[cfg(target_os = "macos")]
    const DEFAULT_DECODER: VideoDecoderType = VideoDecoderType::VideoToolBox;

    #[cfg(target_os = "windows")]
    const DEFAULT_DECODER: VideoDecoderType = VideoDecoderType::D3D11;

    #[cfg(target_os = "linux")]
    const DEFAULT_DECODER: VideoDecoderType = VideoDecoderType::H264;

    fn get_strategy(&self) -> Option<TransportStrategy> {
        Some(match self.strategy.as_ref()?.as_str() {
            "direct" => TransportStrategy::Direct(self.address?),
            "relay" => TransportStrategy::Relay(self.address?),
            "multicast" => TransportStrategy::Multicast(self.address?),
            _ => unreachable!(),
        })
    }

    fn get_video_options(&self) -> VideoOptions {
        VideoOptions {
            codec: self.encoder,
            frame_rate: self.fps,
            width: self.width,
            height: self.height,
            bit_rate: 500 * 1024 * 8,
            key_frame_interval: 21,
        }
    }
}

fn main() -> Result<()> {
    simple_logger::init_with_level(log::Level::Info)?;

    Configure::parse();

    // Creates a message loop, which is used to create the main window.
    let event_loop = EventLoop::new()?;
    event_loop.set_control_flow(ControlFlow::Wait);
    event_loop.run_app(&mut App::default())?;

    // When exiting the application, the environment of hylarana should be cleaned
    // up.
    shutdown()?;
    Ok(())
}
