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

        // Add SPS and PPS units in front of each keyframe.
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

    #[cfg(feature = "frame-drop")]
    loss: AtomicBool,
}

impl VideoStreamReceiverProcesser {
    pub fn new() -> Self {
        Self {
            key_frame: AtomicOption::new(None),
            cfg_ready: AtomicBool::new(false),

            #[cfg(feature = "frame-drop")]
            loss: AtomicBool::new(false),
        }
    }

    pub fn get_key_frame(&self) -> Option<Bytes> {
        self.key_frame.get().cloned()
    }

    /// Marks that the video packet has been lost.
    pub fn loss_pkt(&self) {
        #[cfg(feature = "frame-drop")]
        self.loss.update(true);
    }

    /// As soon as a keyframe is received, the keyframe is cached, and when a
    /// packet loss occurs, the previous keyframe is retransmitted directly into
    /// the decoder.
    pub fn process(&self, buf: Bytes, flags: u8, handle: impl Fn(Bytes) -> bool) -> bool {
        // Get whether a packet has been dropped.
        #[cfg(feature = "frame-drop")]
        let mut is_loss = self.loss.get();
        
        if flags == BufferFlag::KeyFrame as u8 {
            self.key_frame.swap(Some(buf.clone()));

            // When keyframes are received, the video stream can be played back 
            // normally without corruption.
            #[cfg(feature = "frame-drop")]
            if is_loss {
                self.loss.update(false);
                is_loss = false;
            }
        }

        // In case of packet loss, no packet is sent to the decoder.
        #[cfg(feature = "frame-drop")]
        if is_loss {
            return true;
        }

        // Send packets to the decoder only when PPS and SPS units are received, 
        // sending other units to the decoder without configuration information 
        // will generate an error.
        if !self.cfg_ready.get() {
            if flags == BufferFlag::Config as u8 {
                self.cfg_ready.update(true);
            } else {
                return true;
            }
        } else {
            if flags == BufferFlag::Config as u8 {
                return true;
            }
        }

        handle(buf)
    }
}
