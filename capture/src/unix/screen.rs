use crate::{
    CaptureHandler, FrameArrived, Size, Source, SourceType, VideoCaptureSourceDescription,
};

use std::{
    env,
    ptr::{null, null_mut},
    sync::{atomic::AtomicBool, Arc},
    thread, time::Duration,
};

use anyhow::{anyhow, Result};
use frame::VideoFrame;
use libc::{shmat, shmctl, shmdt, shmget, IPC_CREAT, IPC_PRIVATE, IPC_RMID};
use utils::{atomic::EasyAtomic, strings::Strings};
use x11::{
    xlib::{
        XCloseDisplay, XDefaultRootWindow, XDefaultVisual, XDestroyImage, XImage,
        XOpenDisplay, ZPixmap, _XDisplay,
    },
    xshm::{XShmAttach, XShmCreateImage, XShmDetach, XShmGetImage, XShmSegmentInfo},
};

struct Display {
    display: *mut _XDisplay,
    image: *mut XImage,
    info: XShmSegmentInfo,
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

        let mut info = unsafe { std::mem::zeroed::<XShmSegmentInfo>() };
        let image = unsafe {
            XShmCreateImage(
                display,
                XDefaultVisual(display, 0),
                24,
                ZPixmap,
                null_mut(),
                &mut info,
                size.width,
                size.height,
            )
        };

        if image.is_null() {
            unsafe { XCloseDisplay(display) };
            return Err(anyhow!("x11 create image failed"));
        }

        let r_image = unsafe { &mut *image };
        info.shmid = unsafe {
            shmget(
                IPC_PRIVATE,
                r_image.bytes_per_line as usize * r_image.height as usize,
                IPC_CREAT | 0777,
            )
        };

        r_image.data = unsafe { shmat(info.shmid, null(), 0) as *mut _ };
        info.shmaddr = r_image.data;

        if r_image.data.is_null() {
            unsafe {
                XDestroyImage(image);
                XCloseDisplay(display);
                shmctl(info.shmid, IPC_RMID, null_mut());
            }

            return Err(anyhow!("shmat failed"));
        }

        if unsafe { XShmAttach(display, &mut info) } == 0 {
            unsafe {
                XDestroyImage(image);
                XCloseDisplay(display);
                shmdt(info.shmaddr as *const _);
                shmctl(info.shmid, IPC_RMID, null_mut());
            }

            return Err(anyhow!("x11 attch mmap failed"));
        }

        Ok(Self {
            display,
            image,
            info,
        })
    }

    fn capture(&mut self) -> Option<&[u8]> {
        if unsafe {
            XShmGetImage(
                self.display,
                XDefaultRootWindow(self.display),
                self.image,
                0,
                0,
                1,
            )
        } != 0
        {
            let image = unsafe { &*self.image };
            let data_size = (image.bytes_per_line * image.height) as usize;
            Some(unsafe { std::slice::from_raw_parts(image.data as *const u8, data_size) })
        } else {
            None
        }
    }
}

impl Drop for Display {
    fn drop(&mut self) {
        unsafe {
            XShmDetach(self.display, &mut self.info);
            XDestroyImage(self.image);

            shmdt(self.info.shmaddr as *const _);
            shmctl(self.info.shmid, IPC_RMID, null_mut());

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
        thread::Builder::new().name("X11ScreenCaptureThread".to_string()).spawn(move || {
            while status.get() {
                let frame = display.capture().unwrap();

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
