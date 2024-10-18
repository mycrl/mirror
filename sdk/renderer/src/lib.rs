mod audio;
mod video;

use std::{
    ffi::{c_int, c_void},
    ptr::null_mut,
};

use audio::AudioPlayer;
use common::frame::{AudioFrame, VideoFrame};
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
extern "system" fn DllMain(
    _dll_module: u32,
    _call_reason: usize,
    _reserved: *const c_void,
) -> bool {
    #[cfg(debug_assertions)]
    {
        if common::jump_current_exe_dir().is_ok() {
            if common::logger::init("renderer.log", log::LevelFilter::Info).is_err() {
                return false;
            }
        }
    }

    true
}

/// Create the window handle used by the SDK through the original window handle.
#[no_mangle]
#[cfg(target_os = "windows")]
extern "C" fn create_window_handle_for_win32(
    hwnd: *mut c_void,
    width: u32,
    height: u32,
) -> *const WindowHandle {
    assert!(!hwnd.is_null());

    Box::into_raw(Box::new(WindowHandle::Win32(hwnd, width, height)))
}

/// Destroy the window handle without affecting external window handles.
#[no_mangle]
extern "C" fn window_handle_destroy(handle: *const WindowHandle) {
    assert!(!handle.is_null());

    let _ = unsafe { Box::from_raw(handle as *mut WindowHandle) };
}

struct RawRenderer {
    audio: AudioPlayer,
    video: VideoRender,
}

/// Creating a window renderer.
#[no_mangle]
extern "C" fn renderer_create(handle: *const WindowHandle, _backend: i32) -> *mut RawRenderer {
    assert!(!handle.is_null());

    let func = || {
        Ok::<RawRenderer, anyhow::Error>(RawRenderer {
            video: VideoRender::new(unsafe { &*handle })?,
            audio: AudioPlayer::new()?,
        })
    };

    func()
        .map_err(|e| log::error!("{:?}", e))
        .map(|ret| Box::into_raw(Box::new(ret)))
        .unwrap_or_else(|_| null_mut())
}

/// Push the video frame into the renderer, which will update the window
/// texture.
#[no_mangle]
extern "C" fn renderer_on_video(render: *mut RawRenderer, frame: *const VideoFrame) -> bool {
    assert!(!render.is_null() && !frame.is_null());

    unsafe { &mut *render }
        .video
        .send(unsafe { &*frame })
        .map_err(|e| log::error!("{:?}", e))
        .is_ok()
}

/// Push the audio frame into the renderer, which will append to audio queue.
#[no_mangle]
extern "C" fn renderer_on_audio(render: *mut RawRenderer, frame: *const AudioFrame) -> bool {
    assert!(!render.is_null() && !frame.is_null());

    unsafe { &mut *render }
        .audio
        .send(unsafe { &*frame })
        .map_err(|e| log::error!("{:?}", e))
        .is_ok()
}

/// Destroy the window renderer.
#[no_mangle]
extern "C" fn renderer_destroy(render: *mut RawRenderer) {
    assert!(!render.is_null());

    let _ = unsafe { Box::from_raw(render) };
}
