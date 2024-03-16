use bytes::Bytes;
use sync::atomic::AtomicOption;

use crate::BufferFlag;

/// Video Streaming Send Processing
///
/// Because the receiver will normally join the stream in the middle of the
/// stream, and in the face of this situation, it is necessary to process the
/// sps and pps as well as the key frame information.
pub struct VideoStreamSenderProcesser {
    config: AtomicOption<Bytes>,
}

impl VideoStreamSenderProcesser {
    pub fn new() -> Self {
        Self {
            config: AtomicOption::new(None),
        }
    }

    pub fn get_config(&self) -> Option<&[u8]> {
        self.config.get().map(|v| &v[..])
    }

    // h264 decoding any p-frames and i-frames requires sps and pps
    // frames, so the configuration frames are saved here, although it
    // should be noted that the configuration frames will only be
    // generated once.
    pub fn apply(&self, buf: &Bytes, flags: i32) {
        if flags == BufferFlag::Config as i32 {
            self.config.swap(Some(buf.clone()));
        }
    }
}

/// Video Streaming Receiver Processing
///
/// The main purpose is to deal with cases where packet loss occurs at the
/// receiver side, since the SRT communication protocol does not completely
/// guarantee no packet loss.
pub struct VideoStreamReceiverProcesser {
    key_frame: AtomicOption<Bytes>,
}

impl VideoStreamReceiverProcesser {
    pub fn new() -> Self {
        Self {
            key_frame: AtomicOption::new(None),
        }
    }

    pub fn get_key_frame(&self) -> Option<Bytes> {
        self.key_frame.get().cloned()
    }

    // As soon as a keyframe is received, the keyframe is cached, and when a
    // packet loss occurs, the previous keyframe is retransmitted directly into
    // the decoder.
    pub fn apply(&self, buf: &Bytes, flags: u8) {
        if flags == BufferFlag::KeyFrame as u8 {
            self.key_frame.swap(Some(buf.clone()));
        }
    }
}
