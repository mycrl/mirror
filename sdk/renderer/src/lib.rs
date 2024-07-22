mod audio;
mod video;

use std::{ffi::c_int, ptr::null_mut};

use audio::AudioPlayer;
use common::{
    frame::{AudioFrame, VideoFrame},
    jump_current_exe_dir,
};
use video::{Size, VideoRender, WindowHandle};

#[repr(C)]
struct RawSize {
    width: c_int,
    height: c_int,
}

impl From<RawSize> for Size {
    fn from(val: RawSize) -> Self {
        Self {
            width: val.width as u32,
            height: val.height as u32,
        }
    }
}

#[no_mangle]
#[cfg(target_os = "windows")]
extern "C" fn renderer_create_window_handle(
    hwnd: *mut std::ffi::c_void,
    _hinstance: *mut std::ffi::c_void,
) -> *const WindowHandle {
    assert!(!hwnd.is_null());

    Box::into_raw(Box::new(WindowHandle::Win32(hwnd)))
}

#[no_mangle]
extern "C" fn renderer_window_handle_destroy(handle: *const WindowHandle) {
    assert!(!handle.is_null());

    let _ = unsafe { Box::from_raw(handle as *mut WindowHandle) };
}

struct RawRenderer {
    audio: AudioPlayer,
    video: VideoRender,
}

#[no_mangle]
extern "C" fn renderer_create(size: RawSize, handle: *const WindowHandle) -> *mut RawRenderer {
    assert!(!handle.is_null());

    #[cfg(debug_assertions)]
    if jump_current_exe_dir().is_ok() {
        common::logger::init("renderer.log", log::LevelFilter::Info).unwrap();
    }

    let func = || {
        Ok::<RawRenderer, anyhow::Error>(RawRenderer {
            video: VideoRender::new(size.into(), unsafe { &*handle })?,
            audio: AudioPlayer::new()?,
        })
    };

    func()
        .map(|ret| Box::into_raw(Box::new(ret)))
        .unwrap_or_else(|_| null_mut())
}

#[no_mangle]
extern "C" fn renderer_on_video(render: *mut RawRenderer, frame: *const VideoFrame) -> bool {
    assert!(!render.is_null() && !frame.is_null());

    unsafe { &mut *render }
        .video
        .send(unsafe { &*frame })
        .is_ok()
}

#[no_mangle]
extern "C" fn renderer_on_audio(render: *mut RawRenderer, frame: *const AudioFrame) -> bool {
    assert!(!render.is_null() && !frame.is_null());

    unsafe { &mut *render }.audio.send(1, unsafe { &*frame });
    true
}

#[no_mangle]
extern "C" fn renderer_resise(render: *mut RawRenderer, _size: RawSize) -> bool {
    assert!(!render.is_null());

    true
}

#[no_mangle]
extern "C" fn renderer_destroy(render: *mut RawRenderer) {
    assert!(!render.is_null());

    let _ = unsafe { Box::from_raw(render) };
}
