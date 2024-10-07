//! Describe the structure of audio and video data
//!
//! It should be noted that pointers to internal data are temporary. If you need
//! to hold them for a long time, you need to actively copy the data pointed to
//! by the pointer. Therefore, the passed VideoFrame or AudioFrame are temporary
//! references, and there will be no situation where a static structure is
//! passed.

use std::{ffi::c_void, ptr::null};

/// Pulse-code modulation
///
/// Pulse-code modulation (PCM) is a method used to digitally represent analog
/// signals. It is the standard form of digital audio in computers, compact
/// discs, digital telephony and other digital audio applications. In a PCM
/// stream, the amplitude of the analog signal is sampled at uniform intervals,
/// and each sample is quantized to the nearest value within a range of digital
/// steps.
///
/// Linear pulse-code modulation (LPCM) is a specific type of PCM in which the
/// quantization levels are linearly uniform. This is in contrast to PCM
/// encodings in which quantization levels vary as a function of amplitude (as
/// with the A-law algorithm or the μ-law algorithm). Though PCM is a more
/// general term, it is often used to describe data encoded as LPCM.
///
/// A PCM stream has two basic properties that determine the stream's fidelity
/// to the original analog signal: the sampling rate, which is the number of
/// times per second that samples are taken; and the bit depth, which determines
/// the number of possible digital values that can be used to represent each
/// sample.
#[repr(C)]
#[derive(Debug)]
pub struct AudioFrame {
    pub sample_rate: u32,
    pub frames: u32,
    pub data: *const i16,
}

unsafe impl Sync for AudioFrame {}
unsafe impl Send for AudioFrame {}

impl Default for AudioFrame {
    fn default() -> Self {
        Self {
            frames: 0,
            data: null(),
            sample_rate: 0,
        }
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VideoFormat {
    BGRA,
    RGBA,
    NV12,
    I420,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VideoSubFormat {
    D3D11,
    SW,
}

/// YCbCr (NV12)
///
/// YCbCr, Y′CbCr, or Y Pb/Cb Pr/Cr, also written as YCBCR or Y′CBCR, is a
/// family of color spaces used as a part of the color image pipeline in video
/// and digital photography systems. Y′ is the luma component and CB and CR are
/// the blue-difference and red-difference chroma components. Y′ (with prime) is
/// distinguished from Y, which is luminance, meaning that light intensity is
/// nonlinearly encoded based on gamma corrected RGB primaries.
///
/// Y′CbCr color spaces are defined by a mathematical coordinate transformation
/// from an associated RGB primaries and white point. If the underlying RGB
/// color space is absolute, the Y′CbCr color space is an absolute color space
/// as well; conversely, if the RGB space is ill-defined, so is Y′CbCr. The
/// transformation is defined in equations 32, 33 in ITU-T H.273. Nevertheless
/// that rule does not apply to P3-D65 primaries used by Netflix with
/// BT.2020-NCL matrix, so that means matrix was not derived from primaries, but
/// now Netflix allows BT.2020 primaries (since 2021). The same happens with
/// JPEG: it has BT.601 matrix derived from System M primaries, yet the
/// primaries of most images are BT.709.
#[repr(C)]
#[derive(Debug)]
pub struct VideoFrame {
    pub format: VideoFormat,
    pub sub_format: VideoSubFormat,
    pub width: u32,
    pub height: u32,
    pub data: [*const c_void; 3],
    pub linesize: [usize; 3],
}

unsafe impl Sync for VideoFrame {}
unsafe impl Send for VideoFrame {}

impl Default for VideoFrame {
    fn default() -> Self {
        Self {
            width: 0,
            height: 0,
            linesize: [0, 0, 0],
            data: [null(), null(), null()],
            format: VideoFormat::RGBA,
            sub_format: VideoSubFormat::SW,
        }
    }
}
