use bytes::Bytes;
use sync::atomic::AtomicOption;

use crate::BufferFlag;

/// Video Streaming Send Processing
///
/// Because the receiver will normally join the stream in the middle of the
/// stream, and in the face of this situation, it is necessary to process the
/// sps and pps as well as the key frame information.
pub struct VideoStreamSenderProcesser {
    config_buffer: AtomicOption<Bytes>,
}

impl VideoStreamSenderProcesser {
    pub fn new() -> Self {
        Self {
            config_buffer: AtomicOption::new(None),
        }
    }

    pub fn get_config_buffer(&self) -> Option<&[u8]> {
        self.config_buffer.get().map(|v| &v[..])
    }

    pub fn apply(&self, buf: Bytes, flags: i32) {
        if flags == BufferFlag::Config as i32 {
            // h264 decoding any p-frames and i-frames requires sps and pps
            // frames, so the configuration frames are saved here, although it
            // should be noted that the configuration frames will only be
            // generated once.
            self.config_buffer.swap(Some(buf));
        }
    }
}
