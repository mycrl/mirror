use crate::Size;

use std::ptr::null;

use core_video_sys::{
    kCVPixelBufferLock_ReadOnly, CVPixelBufferGetBaseAddressOfPlane,
    CVPixelBufferGetBytesPerRowOfPlane, CVPixelBufferGetHeight, CVPixelBufferGetWidth,
    CVPixelBufferLockBaseAddress, CVPixelBufferUnlockBaseAddress,
};

pub use core_video_sys::CVPixelBufferRef;

pub struct PixelBufferRef {
    size: Size,
    data: [*const u8; 2],
    linesize: [usize; 2],
    buffer: CVPixelBufferRef,
}

impl PixelBufferRef {
    pub fn size(&self) -> Size {
        self.size
    }

    pub fn data(&self) -> &[*const u8; 2] {
        &self.data
    }

    pub fn linesize(&self) -> &[usize; 2] {
        &self.linesize
    }
}

impl From<CVPixelBufferRef> for PixelBufferRef {
    fn from(buffer: CVPixelBufferRef) -> Self {
        unsafe {
            CVPixelBufferLockBaseAddress(buffer, kCVPixelBufferLock_ReadOnly);
        }

        let mut this = Self {
            size: Size {
                width: unsafe { CVPixelBufferGetWidth(buffer) } as u32,
                height: unsafe { CVPixelBufferGetHeight(buffer) } as u32,
            },
            buffer,
            data: [null(); 2],
            linesize: [0; 2],
        };

        for i in 0..2 {
            this.data[i] = unsafe { CVPixelBufferGetBaseAddressOfPlane(buffer, i) as *const _ };
            this.linesize[i] = unsafe { CVPixelBufferGetBytesPerRowOfPlane(buffer, i) };
        }

        this
    }
}

impl Drop for PixelBufferRef {
    fn drop(&mut self) {
        unsafe {
            CVPixelBufferUnlockBaseAddress(self.buffer, kCVPixelBufferLock_ReadOnly);
        }
    }
}
