mod audio;
mod video;

use std::{
    ffi::{c_int, c_void},
    ptr::null_mut,
};

use audio::AudioPlayer;
use sdl2::sys::SDL_Event;
use utils::logger;

use frame::{AudioFrame, VideoFrame};
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

/// Windows yes! The Windows dynamic library has an entry, so just initialize
/// the logger and set the process priority at the entry.
#[no_mangle]
extern "system" fn DllMain(_module: u32, call_reason: usize, _reserved: *const c_void) -> bool {
    match call_reason {
        1 /* DLL_PROCESS_ATTACH */ => renderer_startup(),
        _ => true,
    }
}

/// Initialize the environment, which must be initialized before using the SDK.
#[no_mangle]
extern "C" fn renderer_startup() -> bool {
    logger::init(
        log::LevelFilter::Info,
        if cfg!(debug_assertions) {
            Some("renderer.log")
        } else {
            None
        },
    )
    .is_ok()
}

/// Create the window handle used by the SDK through the original window handle.
#[no_mangle]
#[cfg(target_os = "windows")]
extern "C" fn renderer_create_window_handle(
    hwnd: *mut c_void,
    _hinstance: *mut c_void,
) -> *const WindowHandle {
    assert!(!hwnd.is_null());

    Box::into_raw(Box::new(WindowHandle::Win32(hwnd)))
}

/// Destroy the window handle without affecting external window handles.
#[no_mangle]
extern "C" fn renderer_window_handle_destroy(handle: *const WindowHandle) {
    assert!(!handle.is_null());

    let _ = unsafe { Box::from_raw(handle as *mut WindowHandle) };
}

struct RawRenderer {
    audio: AudioPlayer,
    video: VideoRender,
}

/// Creating a window renderer.
#[no_mangle]
extern "C" fn renderer_create(size: RawSize, handle: *const WindowHandle) -> *mut RawRenderer {
    let func = || {
        Ok::<RawRenderer, anyhow::Error>(RawRenderer {
            video: VideoRender::new(
                size.into(),
                if handle.is_null() {
                    Some(unsafe { &*handle })
                } else {
                    None
                },
            )?,
            audio: AudioPlayer::new()?,
        })
    };

    func()
        .map_err(|e| log::error!("{:?}", e))
        .map(|ret| Box::into_raw(Box::new(ret)))
        .unwrap_or_else(|_| null_mut())
}

/// Wait indefinitely for the next available event.
#[no_mangle]
extern "C" fn renderer_event_loop(
    render: *mut RawRenderer,
    handle: Option<extern "C" fn(event: *const c_void, ctx: *const c_void) -> bool>,
    ctx: *const c_void,
) {
    assert!(!render.is_null() && handle.is_some());

    let func = handle.unwrap();
    unsafe { &mut *render }.video.start_loop(move |event| {
        func(
            unsafe { std::mem::transmute::<*const SDL_Event, *const c_void>(event) },
            ctx,
        )
    });
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

/// Adjust the size of the renderer. When the window size changes, the internal
/// size of the renderer needs to be updated, otherwise this will cause abnormal
/// rendering.
#[no_mangle]
extern "C" fn renderer_resise(render: *mut RawRenderer, _size: RawSize) -> bool {
    assert!(!render.is_null());

    true
}

/// Destroy the window renderer.
#[no_mangle]
extern "C" fn renderer_destroy(render: *mut RawRenderer) {
    assert!(!render.is_null());

    let _ = unsafe { Box::from_raw(render) };
}
