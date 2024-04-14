//! # NV12 Video Frame

use std::slice::from_raw_parts;

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct FrameRect {
    pub width: usize,
    pub height: usize,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct VideoFrame {
    pub rect: FrameRect,
    pub data: [*const u8; 2],
    pub linesize: [usize; 2],
}

impl VideoFrame {
    pub fn new(data: [&[u8]; 2], linesize: [usize; 2], rect: FrameRect) -> Self {
        Self {
            rect,
            linesize,
            data: [data[0].as_ptr(), data[1].as_ptr()],
        }
    }

    pub fn get_y_planar<'a>(&'a self) -> &'a [u8] {
        unsafe { from_raw_parts(self.data[0], self.linesize[0] * self.rect.height) }
    }

    pub fn get_uv_planar<'a>(&'a self) -> &'a [u8] {
        unsafe { from_raw_parts(self.data[1], self.linesize[1] * self.rect.height) }
    }
}
