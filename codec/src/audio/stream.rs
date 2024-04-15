use bytes::Bytes;
use common::atomic::AtomicOption;

use crate::BufferFlag;

/// Audio Streaming Send Processing
///
/// Because the receiver will normally join the stream in the middle of the
/// stream, and in the face of this situation, it is necessary to process the
/// config information.
pub struct AudioStreamSenderProcesser {
    config: AtomicOption<Bytes>,
}

impl AudioStreamSenderProcesser {
    pub fn new() -> Self {
        Self {
            config: AtomicOption::new(None),
        }
    }

    pub fn get_config(&self) -> Option<&[u8]> {
        self.config.get().map(|v| &v[..])
    }

    // opus decoding any p-frames and i-frames requires config
    // information, so the configuration frames are saved here, although it
    // should be noted that the configuration frames will only be
    // generated once.
    pub fn apply(&self, buf: &Bytes, flags: i32) {
        if flags == BufferFlag::Config as i32 {
            self.config.swap(Some(buf.clone()));
        }
    }
}
