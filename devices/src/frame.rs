use crate::{api::VideoFrame, VideoFormat, VideoInfo};

#[derive(Debug)]
pub struct Frame<'a> {
    pub data: [&'a [u8]; 3],
    pub timestamp: u64,
}

impl<'a> Frame<'a> {
    pub(crate) fn from_raw(value: *const VideoFrame, info: &VideoInfo) -> Self {
        let frame = unsafe { &*value };
        let mut data: [&[u8]; 3] = [&[]; 3];

        if info.format == VideoFormat::VIDEO_FORMAT_BGRA
            || info.format == VideoFormat::VIDEO_FORMAT_RGBA
        {
            data[0] = unsafe {
                std::slice::from_raw_parts(
                    frame.data[0],
                    info.width as usize * info.height as usize * 4,
                )
            };
        } else if info.format == VideoFormat::VIDEO_FORMAT_NV12 {
            data[0] = unsafe { std::slice::from_raw_parts(frame.data[0], info.width as usize) };
            data[1] = unsafe {
                std::slice::from_raw_parts(
                    frame.data[1],
                    (info.width as usize / 2) * (info.height as usize / 2) * 2,
                )
            };
        } else if info.format == VideoFormat::VIDEO_FORMAT_I420 {
            data[0] = unsafe { std::slice::from_raw_parts(frame.data[0], info.width as usize) };
            data[1] = unsafe {
                std::slice::from_raw_parts(
                    frame.data[1],
                    (info.width as usize / 2) * (info.height as usize / 2),
                )
            };

            data[2] = unsafe {
                std::slice::from_raw_parts(
                    frame.data[2],
                    (info.width as usize / 2) * (info.height as usize / 2),
                )
            };
        }

        Self {
            timestamp: frame.timestamp,
            data,
        }
    }
}
