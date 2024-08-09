mod camera;
mod screen;

pub use self::{camera::CameraCapture, screen::ScreenCapture};

use anyhow::Result;
use windows::Win32::{
    Media::MediaFoundation::{MFShutdown, MFStartup, MF_VERSION},
    System::Com::{CoInitializeEx, CoUninitialize, COINIT_MULTITHREADED},
};

/// Initializes Microsoft Media Foundation.
pub fn startup() -> Result<()> {
    unsafe {
        CoInitializeEx(None, COINIT_MULTITHREADED).ok()?;
        MFStartup(MF_VERSION, 0)?;
    }

    Ok(())
}

/// Shuts down the Microsoft Media Foundation platform. Call this function
/// once for every call to MFStartup. Do not call this function from work
/// queue threads.
pub fn shutdown() -> Result<()> {
    unsafe {
        MFShutdown()?;
        CoUninitialize();
    }

    Ok(())
}
