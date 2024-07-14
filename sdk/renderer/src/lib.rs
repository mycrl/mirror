mod audio;
mod video;

use std::{
    ffi::{c_int, c_void},
    num::NonZeroIsize,
    ptr::null_mut,
};

use audio::AudioPlayer;
use common::frame::{AudioFrame, VideoFrame};
use video::{Size, VideoRender, WindowHandle};
use wgpu::rwh::Win32WindowHandle;

#[repr(C)]
struct RawSize {
    width: c_int,
    height: c_int,
}

impl Into<Size> for RawSize {
    fn into(self) -> Size {
        Size {
            width: self.width as u32,
            height: self.height as u32,
        }
    }
}

#[no_mangle]
extern "C" fn renderer_create_window_handle(
    hwnd: *mut c_void,
    hinstance: *mut c_void,
) -> *const WindowHandle {
    assert!(!hwnd.is_null());
    assert!(!hinstance.is_null());

    let mut handle = Win32WindowHandle::new(NonZeroIsize::new(hwnd as isize).unwrap());
    handle.hinstance = Some(NonZeroIsize::new(hinstance as isize).unwrap());
    Box::into_raw(Box::new(WindowHandle::Win32(handle)))
}

#[no_mangle]
extern "C" fn renderer_window_handle_destroy(handle: *const WindowHandle) {
    assert!(!handle.is_null());

    let _ = unsafe { Box::from_raw(handle as *mut WindowHandle) };
}

struct RawRenderer {
    audio: AudioPlayer,
    // video: VideoRender<'static>,
}

#[no_mangle]
extern "C" fn renderer_create(size: RawSize, handle: *const WindowHandle) -> *const RawRenderer {
    assert!(!handle.is_null());

    let func = || {
        Ok::<RawRenderer, anyhow::Error>(RawRenderer {
            // video: VideoRender::new(size.into(), unsafe { &*handle })?,
            audio: AudioPlayer::new()?,
        })
    };

    func()
        .map(|ret| Box::into_raw(Box::new(ret)))
        .unwrap_or_else(|_| null_mut())
}

#[no_mangle]
extern "C" fn renderer_on_video(render: *const RawRenderer, frame: *const VideoFrame) -> bool {
    assert!(!render.is_null() && !frame.is_null());

    // unsafe { &*render }.video.send(unsafe { &*frame }).is_ok()
    true
}

#[no_mangle]
extern "C" fn renderer_on_audio(render: *const RawRenderer, frame: *const AudioFrame) -> bool {
    assert!(!render.is_null() && !frame.is_null());

    unsafe { &*render }.audio.send(1, unsafe { &*frame });
    true
}

#[no_mangle]
extern "C" fn renderer_resise(render: *const RawRenderer, size: RawSize) -> bool {
    assert!(!render.is_null());

    // unsafe { &*render }.video.resize(size.into()).is_ok()
    true
}

#[no_mangle]
extern "C" fn renderer_destroy(render: *const RawRenderer) {
    assert!(!render.is_null());

    let _ = unsafe { Box::from_raw(render as *mut RawRenderer) };
}
