use std::ptr::null_mut;

use anyhow::{anyhow, Result};
use ffmpeg_sys_next::*;

struct Capture(*mut AVFormatContext);

unsafe impl Send for Capture {}
unsafe impl Sync for Capture {}

impl Capture {
    fn new() -> Result<Self> {
        let mut ctx = null_mut();
        if unsafe {
            avformat_open_input(
                &mut ctx,
                "/dev/dri/card0".as_ptr() as *const _,
                av_find_input_format("kmsgrab".as_ptr() as *const _),
                null_mut(),
            )
        } != 0
        {
            return Err(anyhow!("not open kms device"));
        }

        if unsafe { avformat_find_stream_info(ctx, null_mut()) } != 0 {
            return Err(anyhow!("not found kms device capture stream"));
        }

        Ok(Self(ctx))
    }
}

impl Drop for Capture {
    fn drop(&mut self) {
        unsafe {
            avformat_close_input(&mut self.0);
        }
    }
}

fn main() -> anyhow::Result<()> {
    Capture::new()?;
    Ok(())
}
