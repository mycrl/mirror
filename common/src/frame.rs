#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct VideoFrameRect {
    pub width: usize,
    pub height: usize,
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
#[derive(Debug, Clone, Copy)]
pub struct VideoFrame {
    pub rect: VideoFrameRect,
    pub data: [*const u8; 2],
    pub linesize: [usize; 2],
}

#[repr(C)]
#[allow(non_camel_case_types)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AudioFormat {
    AUDIO_NONE = -1,
    AUDIO_U8,          // unsigned 8 bits
    AUDIO_S16,         // signed 16 bits
    AUDIO_S32,         // signed 32 bits
    AUDIO_FLT,         // float
    AUDIO_DBL,         // double
    AUDIO_U8P,         // unsigned 8 bits, planar
    AUDIO_S16P,        // signed 16 bits, planar
    AUDIO_S32P,        // signed 32 bits, planar
    AUDIO_FLTP,        // float, planar
    AUDIO_DBLP,        // double, planar
    AUDIO_S64,         // signed 64 bits
    AUDIO_S64P,        // signed 64 bits, planar
    AUDIO_NB           // Number of sample formats.
}

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
#[derive(Debug, Clone, Copy)]
pub struct AudioFrame {
    pub format: AudioFormat,
    pub frames: u32,
    pub data: *const u8,
}
