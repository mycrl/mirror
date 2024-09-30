use crate::{
    CaptureHandler, FrameArrived, Size, Source, SourceType, VideoCaptureSourceDescription,
};

use std::{
    env,
    fs::{File, OpenOptions},
    os::fd::{AsFd, BorrowedFd},
    ptr::null_mut,
    sync::{atomic::AtomicBool, Arc},
    thread::{self, sleep},
    time::Duration,
};

use anyhow::{anyhow, Result};
use drm::{
    control::{
        framebuffer::{Info, PlanarInfo},
        plane, Device as DrmControlDevice,
    },
    ClientCapability, Device as DrmDevice,
};
use frame::VideoFrame;
use utils::{atomic::EasyAtomic, strings::Strings};

#[derive(Default)]
pub struct ScreenCapture(Arc<AtomicBool>);

impl CaptureHandler for ScreenCapture {
    type Frame = VideoFrame;
    type Error = anyhow::Error;
    type CaptureDescriptor = VideoCaptureSourceDescription;

    fn get_sources() -> Result<Vec<Source>, Self::Error> {
        Ok(vec![Source {
            index: 0,
            is_default: true,
            kind: SourceType::Screen,
            id: "default display".to_string(),
            name: "default display".to_string(),
        }])
    }

    fn start<S: FrameArrived<Frame = Self::Frame> + 'static>(
        &self,
        options: Self::CaptureDescriptor,
        mut arrived: S,
    ) -> Result<(), Self::Error> {
        let mut capture = Capture::new()?;

        thread::Builder::new()
            .name("LinuxScreenCaptureThread".to_string())
            .spawn(move || {
                while let Some(frame) = capture.read() {
                    sleep(Duration::from_millis(1000 / options.fps as u64));
                }
            })?;

        Ok(())
    }

    fn stop(&self) -> Result<(), Self::Error> {
        self.0.update(false);
        Ok(())
    }
}

struct Card(File);

impl DrmDevice for Card {}
impl DrmControlDevice for Card {}

impl AsFd for Card {
    fn as_fd(&self) -> BorrowedFd<'_> {
        self.0.as_fd()
    }
}

struct Capture {
    card: Card,
    framebuffer: Info,
    planar_info: PlanarInfo,
}

impl Capture {
    fn new() -> Result<Self> {
        let card = Card(
            OpenOptions::new()
                .read(true)
                .write(true)
                .open("/dev/dri/card0")?,
        );

        // The driver provides more plane types for modesetting
        card.set_client_capability(ClientCapability::UniversalPlanes, true);

        // find frame buffer and planar info
        let framebuffer = card
            .resource_handles()?
            .crtcs()
            .iter()
            .map(|it| card.get_crtc(*it))
            .filter(|it| it.is_ok())
            .map(|it| it.unwrap())
            .find(|it| it.framebuffer().is_some())
            .map(|it| it.framebuffer().unwrap());

        let (framebuffer, planar_info) = if let Some(handle) = framebuffer {
            (
                card.get_framebuffer(handle)?,
                card.get_planar_framebuffer(handle)?,
            )
        } else {
            return Err(anyhow!("not found a frame buffer"));
        };

        Ok(Self {
            framebuffer,
            planar_info,
            card,
        })
    }

    fn size(&self) -> Size {
        let (width, height) = self.framebuffer.size();
        Size { width, height }
    }

    fn read(&self) -> Result<()> {
        Ok(())
    }
}
