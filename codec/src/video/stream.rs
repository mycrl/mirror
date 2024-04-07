use std::sync::atomic::AtomicBool;

use bytes::Bytes;
use sync::atomic::{AtomicOption, EasyAtomic};

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

    // h264 decoding any p-frames and i-frames requires sps and pps
    // frames, so the configuration frames are saved here, although it
    // should be noted that the configuration frames will only be
    // generated once.
    pub fn process(&self, buf: Bytes, flags: i32, handle: impl Fn(Bytes, u8) -> bool) -> bool {
        if flags == BufferFlag::Config as i32 {
            self.config.swap(Some(buf.clone()));
        }

        if flags == BufferFlag::KeyFrame as i32 {
            if !handle(
                self.config
                    .get()
                    .map(|v| v.clone())
                    .unwrap_or_else(|| Bytes::new()),
                BufferFlag::Config as u8,
            ) {
                return false;
            }
        }

        handle(buf, flags as u8)
    }
}

/// Video Streaming Receiver Processing
///
/// The main purpose is to deal with cases where packet loss occurs at the
/// receiver side, since the SRT communication protocol does not completely
/// guarantee no packet loss.
pub struct VideoStreamReceiverProcesser {
    key_frame: AtomicOption<Bytes>,
    cfg_ready: AtomicBool,
}

impl VideoStreamReceiverProcesser {
    pub fn new() -> Self {
        Self {
            key_frame: AtomicOption::new(None),
            cfg_ready: AtomicBool::new(false),
        }
    }

    pub fn get_key_frame(&self) -> Option<Bytes> {
        self.key_frame.get().cloned()
    }

    // As soon as a keyframe is received, the keyframe is cached, and when a
    // packet loss occurs, the previous keyframe is retransmitted directly into
    // the decoder.
    pub fn process(&self, buf: Bytes, flags: u8, handle: impl Fn(Bytes) -> bool) -> bool {
        if flags == BufferFlag::KeyFrame as u8 {
            self.key_frame.swap(Some(buf.clone()));
        }

        if flags == BufferFlag::Config as u8 {
            if !self.cfg_ready.get() {
                self.cfg_ready.update(true);
            } else {
                return true;
            }
        }

        handle(buf)
    }
}
