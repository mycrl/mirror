use std::slice::from_raw_parts;

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct VideoFrameRect {
    pub width: usize,
    pub height: usize,
}

/// nv12 format
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct VideoFrame {
    pub rect: VideoFrameRect,
    pub data: [*const u8; 2],
    pub linesize: [usize; 2],
}

impl VideoFrame {
    pub fn new(data: [&[u8]; 2], linesize: [usize; 2], rect: VideoFrameRect) -> Self {
        Self {
            data: [data[0].as_ptr(), data[1].as_ptr()],
            linesize,
            rect,
        }
    }

    #[inline]
    pub fn get_y_planar<'a>(&'a self) -> &'a [u8] {
        unsafe { from_raw_parts(self.data[0], self.linesize[0] * self.rect.height) }
    }

    #[inline]
    pub fn get_uv_planar<'a>(&'a self) -> &'a [u8] {
        unsafe { from_raw_parts(self.data[1], self.linesize[1] * self.rect.height) }
    }
}

/// pcm format
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct AudioFrame {
    pub frames: u32,
    pub data: [*const u8; 2],
}
