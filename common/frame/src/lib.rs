//! Describe the structure of audio and video data
//!
//! It should be noted that pointers to internal data are temporary. If you need
//! to hold them for a long time, you need to actively copy the data pointed to
//! by the pointer. Therefore, the passed VideoFrame or AudioFrame are temporary
//! references, and there will be no situation where a static structure is
//! passed.

mod audio;
mod video;

pub use self::{
    audio::{AudioFrame, AudioResampler},
    video::{VideoFormat, VideoFrame, VideoSize},
};

#[cfg(target_os = "windows")]
pub use self::video::win32::{Resource, TextureBuffer, VideoTransform, VideoTransformDescriptor};

#[cfg(target_os = "linux")]
pub use self::video::unix::VideoTransform;
