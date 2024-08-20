use crate::{
    CaptureHandler, FrameArrived, Size, Source, SourceType, VideoCaptureSourceDescription,
};

use std::{
    env,
    sync::{atomic::AtomicBool, Arc},
    thread,
    time::Duration,
};

use anyhow::{anyhow, Result};
use frame::{VideoFormat, VideoFrame, VideoSize, VideoTransform};
use utils::{atomic::EasyAtomic, strings::Strings};
use x11::xlib::{
    XAllPlanes, XCloseDisplay, XDefaultRootWindow, XDestroyImage, XGetImage, XGetWindowAttributes,
    XImage, XOpenDisplay, XWindowAttributes, ZPixmap, _XDisplay,
};

struct Display {
    input: Size,
    output: Size,
    root: u64,
    display: *mut _XDisplay,
}

unsafe impl Send for Display {}
unsafe impl Sync for Display {}

impl Display {
    fn new(size: Size) -> Result<Self> {
        let name = Strings::from(env::var("DISPLAY")?.as_str());
        let display = unsafe { XOpenDisplay(name.as_ptr()) };
        if display.is_null() {
            return Err(anyhow!("x11 open display failed"));
        }

        let root = unsafe { XDefaultRootWindow(display) };

        let mut attr = unsafe { std::mem::zeroed::<XWindowAttributes>() };
        unsafe {
            XGetWindowAttributes(display, root, &mut attr);
        }

        Ok(Self {
            root,
            display,
            output: size,
            input: Size {
                width: attr.width as u32,
                height: attr.height as u32,
            },
        })
    }

    fn capture(&mut self) -> Option<Texture> {
        let image = unsafe {
            XGetImage(
                self.display,
                self.root,
                0,
                0,
                self.input.width,
                self.input.height,
                XAllPlanes(),
                ZPixmap,
            )
        };

        if !image.is_null() {
            Some(Texture(image))
        } else {
            None
        }
    }
}

struct Texture(*mut XImage);

impl AsRef<[u8]> for Texture {
    fn as_ref(&self) -> &[u8] {
        let image = unsafe { &*self.0 };
        let data_size = (image.bytes_per_line * image.height) as usize;
        unsafe { std::slice::from_raw_parts(image.data as *const u8, data_size) }
    }
}

impl Drop for Texture {
    fn drop(&mut self) {
        unsafe {
            XDestroyImage(self.0);
        }
    }
}

impl Drop for Display {
    fn drop(&mut self) {
        unsafe {
            XCloseDisplay(self.display);
        }
    }
}

#[derive(Default)]
pub struct ScreenCapture(Arc<AtomicBool>);

impl CaptureHandler for ScreenCapture {
    type Frame = VideoFrame;
    type Error = anyhow::Error;
    type CaptureOptions = VideoCaptureSourceDescription;

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
        options: Self::CaptureOptions,
        mut arrived: S,
    ) -> Result<(), Self::Error> {
        let mut display = Display::new(options.size)?;

        self.0.update(true);

        let status = self.0.clone();
        thread::Builder::new()
            .name("X11ScreenCaptureThread".to_string())
            .spawn(move || {
                let mut processor = VideoTransform::new(
                    VideoSize {
                        width: display.input.width,
                        height: display.input.height,
                    },
                    VideoSize {
                        width: display.output.width,
                        height: display.output.height,
                    },
                );

                let mut frame = VideoFrame::default();
                frame.width = options.size.width;
                frame.height = options.size.height;
                frame.linesize = [frame.width as usize, frame.width as usize];

                while status.get() {
                    let data = display.capture().unwrap();
                    let texture = processor.process(data.as_ref(), VideoFormat::ARGB);

                    frame.data[0] = texture.as_ptr();
                    frame.data[1] =
                        unsafe { texture.as_ptr().add((frame.width * frame.height) as usize) };

                    if !arrived.sink(&frame) {
                        break;
                    }

                    thread::sleep(Duration::from_millis(1000 / options.fps as u64));
                }

                status.update(false);
            })?;

        Ok(())
    }

    fn stop(&self) -> Result<(), Self::Error> {
        self.0.update(false);
        Ok(())
    }
}
