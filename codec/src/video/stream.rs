use bytes::Bytes;
use sync::atomic::AtomicOption;

use crate::BufferFlag;

pub struct VideoStreamSenderProcesser {
    config_buffer: AtomicOption<Bytes>,
    key_buffer: AtomicOption<Bytes>,
}

impl VideoStreamSenderProcesser {
    pub fn new() -> Self {
        Self {
            config_buffer: AtomicOption::new(None),
            key_buffer: AtomicOption::new(None),
        }
    }

    pub fn get_config_buffer(&self) -> Option<&[u8]> {
        self.config_buffer.get().map(|v| &v[..])
    }

    pub fn get_key_buffer(&self) -> Option<&[u8]> {
        self.key_buffer.get().map(|v| &v[..])
    }

    pub fn apply(&self, buf: Bytes, flags: i32) {
        if flags == BufferFlag::Config as i32 {
            self.config_buffer.swap(Some(buf));
        } else if flags == BufferFlag::KeyFrame as i32 {
            self.key_buffer.swap(Some(buf));
        }
    }
}
