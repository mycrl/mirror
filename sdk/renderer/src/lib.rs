mod audio;
mod video;

use std::{
    ffi::{c_int, c_void},
    ptr::null_mut,
};

use anyhow::anyhow;
use audio::AudioPlayer;
use frame::{AudioFrame, VideoFrame};
use utils::{logger, win32::Direct3DDevice};
use video::{Size, VideoRender, VideoRenderOptions};
use windows::{
    core::Interface,
    Win32::{
        Foundation::HWND,
        Graphics::Direct3D11::{ID3D11Device, ID3D11DeviceContext},
    },
};

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
    std::panic::set_hook(Box::new(|info| {
        log::error!("{:?}", info);
    }));

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

#[repr(C)]
struct RawRenderer {
    audio: AudioPlayer,
    video: VideoRender,
}

#[repr(C)]
struct RawRendererOptions {
    size: RawSize,
    hwnd: *mut c_void,
    d3d_device: *mut c_void,
    d3d_device_context: *mut c_void,
}

/// Creating a window renderer.
#[no_mangle]
extern "C" fn renderer_create(options: RawRendererOptions) -> *mut RawRenderer {
    let func = || {
        Ok::<RawRenderer, anyhow::Error>(RawRenderer {
            audio: AudioPlayer::new()?,
            video: VideoRender::new(VideoRenderOptions {
                size: options.size.into(),
                window_handle: HWND(options.hwnd),
                direct3d: Direct3DDevice {
                    device: unsafe {
                        ID3D11Device::from_raw_borrowed(&options.d3d_device)
                            .ok_or_else(|| anyhow!("invalid d3d11 device"))?
                            .clone()
                    },
                    context: unsafe {
                        ID3D11DeviceContext::from_raw_borrowed(&options.d3d_device_context)
                            .ok_or_else(|| anyhow!("invalid d3d11 device context"))?
                            .clone()
                    },
                },
            })?,
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
