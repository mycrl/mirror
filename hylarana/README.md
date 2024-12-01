A development toolkit for writing low-latency and cross-platform screen casting applications.

Hylarana is a cross-platform screen casting SDK for writing screen casting applications using the Rust programming language. It provides a very high level of abstraction, shields cross-platform details, codec support for software hardware, and supports multiple transmission methods. This project prioritizes latency and performance. Currently, the end-to-end latency is controlled below 250ms, the actual latency performance is a bit off depending on platform differences and transmission methods, and the performance is worse on Linux platform, and only software codecs are supported on Linux platform.

-   Transmission method: SRT transmission protocol, UDP multicast.
-   Codec support: Open H264, X264, Intel Qsv, Video Toolbox.

## Quick Start

hylarana does not need to specifically address cross-platform differences and feature flags:

```toml
hylarana = "0.2"
```

### Creating a Sender

The sender is also the initiator of the screen casting, before initiating the screen casting, you need to determine the capture source, here we capture the screen and the system sound as an example.

Get the list of screens:

```rust
let video_sources = Capture::get_sources(SourceType::Screen)?;
```

`sources` is a list of all the screens on your current device, where `is_default` with `true` is your default monitor.

Get the system sound output device:

```rust
let audio_sources = Capture::get_sources(SourceType::Audio)?;
```

The audio output device follows the same rules as the monitor above.

To keep things a bit simpler, we use the first source of the audio source list and the video source list.

```rust
let video_source = video_sources.get(0)?;
let audio_source = audio_sources.get(0)?;
```

Next, create the sender configurations.

Start by creating the encoding configurations for the audio and video sources:

```rust
let video_options = HylaranaSenderTrackOptions {
    source: video_source,
    options: VideoOptions {
        codec: VideoEncoderType::X264,
        frame_rate: 30,
        width: 1280,
        height: 720,
        bit_rate: 500 * 1024 * 8,
        key_frame_interval: 21,
    },
}

let audio_options = HylaranaSenderTrackOptions {
    source: audio_source,
    options: AudioOptions {
        sample_rate: 48000,
        bit_rate: 64000,
    },
};
```

The video encoder uses a software encoder and is fixed at 30 frames per second for `1280x720` video. Audio is fixed at `48khz` sample rate.

Then, create the sender.

First, we use UDP multicast as the network transmission method for screen casting:

```rust
let transport = TransportOptions {
    strategy: TransportStrategy::Multicast("239.0.0.1:8080".parse()?),
    mtu: 1500,
};
```

The sender will send the audio and video packets to port `8080` on `239.0.0.1`.

Pass these configurations to the `create_sender` function to create the sender:

```rust
let sender = Hylarana::create_sender(
    HylaranaSenderOptions {
        transport,
        media: HylaranaSenderMediaOptions {
            video: video_options,
            audio: audio_options,
        },
    },
    view,
)?;
```

You may notice that `view` we haven't created yet, don't worry, we'll go back and create `view` to display our sender preview screen next.

```rust
struct View(Mutex<VideoRender<'a>>);

impl AVFrameStream for View {}

impl AVFrameSink for View {
    fn audio(&self, frame: &AudioFrame) -> bool {
        true
    }

    fn video(&self, frame: &VideoFrame) -> bool {
        self.video.send(frame).is_ok()
    }
}

impl AVFrameObserver for View {
    fn close(&self) {
        println!("view is closed");
    }
}
```

We implement `AVFrameStream` for `View`, which is needed for `create_sender`. We submit every video frame we receive to the `VideoRender`, but the audio we don't process because playing native sound will cause an audio loopback (you don't want that).

Let's go back to how to create the `VideoRender`.

The renderer needs a window to output and display the screen, it is recommended to use `winit` to create a native window, this library is very easy to use, but instead of showing how to create a window with winit and such, we will assume that a `window` has been created:

```rust
let inner_size = window.inner_size();
let video_render = VideoRender::new(VideoRenderOptions {
    backend: VideoRenderBackend::WebGPU,
    target: window,
    size: Size {
        width: inner_size.width,
        height: inner_size.height,
    },
})?;
```

Then create `View`.

```rust
let view = View(video_render);
```

This way, we have a window where we can preview the video screen.

After the creation of the sender is complete, it is important to note that you need to get the unique identifier of the sender via `sender.get_id()`, which is needed by the receiver to find the sender.

### Creating a Receiver

Receiver is used to receive audio and video streams from the sender. The relationship between a receiver and a sender is not one-to-one, and multiple receivers can receive audio and video streams from a sender at the same time.

Creating a receiver is much simpler, and we refer directly to the sender's creation configuration to create the receiver:

```rust
let receiver = Hylarana::create_receiver(
    id,
    HylaranaReceiverOptions {
        codec: HylaranaReceiverCodecOptions {
            video: VideoDecoderType::H264,
        },
        transport: TransportOptions {
            strategy: TransportStrategy::Multicast("239.0.0.1:8080".parse()?),
            mtu: 1500,
        },
    },
    view,
)?;
```

The `id` comes from the sender, for video decoding we use a software decoder, and the transport layer policy needs to be the same on the receiver side as on the sender side, otherwise the two sides won't be able to communicate with each other using different policies. The creation of the `view` has already been implemented in the above section on creating the sender, so we won't implement it here. But the receiving end needs to play the sound, you just need to create one more `AudioRender` and refer to the example above to process the audio frames.

## LAN discovery

Considering that if there is no mechanism for LAN discovery, the creation process between the sender and the receiver requires an external server to intervene and synchronize some signaling and configuration information, which is not possible out of the box. So hylarana has a built-in LAN discovery component, where you can register a service with `DiscoveryService` and pass its description, so that other devices can query the registered service for information.

The `DiscoveryService` provides interfaces for registration and querying. We will combine the example of creating a sender and a receiver below to create a receiver screen by passing configuration information from the sender to the receiver through LAN discovery.

First, we need to create the sender, but we'll skip that here and refer to the Creating the sender section above and then register a service:

```rust
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
struct StreamInfo {
    id: String,
    name: String,
    strategy: TransportStrategy,
}

let service = DiscoveryService::register(3456, &StreamInfo {
    name: "test".to_string(),
    id: sender.get_id().to_string(),
    strategy: TransportStrategy::Multicast("239.0.0.1:8080".parse()?),
})?;
```

`StreamInfo` is the descriptive information of our service, any type that implements `Serialize` and `Deserialize` can be passed as a service property to the `register` method. The port number needs to be defined by yourself, I define it as `3456` here. Once registration is complete, `DiscoveryService` will broadcast the current service from the NIC via the `mdns` protocol.

Next, we turn to the receiving end side:

```rust
let service = DiscoveryService::query(|addrs, info: StreamInfo| {
    if info.name == "test" {
        let receiver = Hylarana::create_receiver(
            info.id,
            HylaranaReceiverOptions {
                codec: HylaranaReceiverCodecOptions {
                    video: VideoDecoderType::H264,
                },
                transport: TransportOptions {
                    strategy: info.strategy,
                    mtu: 1500,
                },
            },
            view,
        ).unwrap();
    }
})?;
```

On the receiver side, we create the receiver by querying for services that have already been registered, calling `DiscoveryService::query` will always listen for service registrations, and will trigger a callback if a service has already been registered, or is in the process of being registered.

The first parameter in the callback is the device address of the service, which is a list because there may be multiple network devices. Here, since our transmission is implemented via UDP multicast, we don't need to be concerned with the network address of the sending end.

The second parameter is the properties of the sender's service. We first make sure that the service was created by `test`, then we get the transport policy and sender ID from the properties and pass it to the sender creation function to create the sender.
