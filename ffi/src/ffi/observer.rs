use std::ffi::c_void;

use hylarana::{AVFrameObserver, AVFrameSink, AVFrameStream, AudioFrame, VideoFrame};

#[repr(C)]
pub(crate) struct RawAVFrameStream {
    /// Callback occurs when the video frame is updated. The video frame
    /// format is fixed to NV12. Be careful not to call blocking
    /// methods inside the callback, which will seriously slow down
    /// the encoding and decoding pipeline.
    ///
    /// YCbCr (NV12)
    ///
    /// YCbCr, Y′CbCr, or Y Pb/Cb Pr/Cr, also written as YCBCR or Y′CBCR, is
    /// a family of color spaces used as a part of the color image
    /// pipeline in video and digital photography systems. Y′ is the
    /// luma component and CB and CR are the blue-difference and
    /// red-difference chroma components. Y′ (with prime) is
    /// distinguished from Y, which is luminance, meaning that light
    /// intensity is nonlinearly encoded based on gamma corrected
    /// RGB primaries.
    ///
    /// Y′CbCr color spaces are defined by a mathematical coordinate
    /// transformation from an associated RGB primaries and white point. If
    /// the underlying RGB color space is absolute, the Y′CbCr color space
    /// is an absolute color space as well; conversely, if the RGB space is
    /// ill-defined, so is Y′CbCr. The transformation is defined in
    /// equations 32, 33 in ITU-T H.273. Nevertheless that rule does not
    /// apply to P3-D65 primaries used by Netflix with BT.2020-NCL matrix,
    /// so that means matrix was not derived from primaries, but now Netflix
    /// allows BT.2020 primaries (since 2021). The same happens with
    /// JPEG: it has BT.601 matrix derived from System M primaries, yet the
    /// primaries of most images are BT.709.
    pub(crate) video: Option<extern "C" fn(ctx: *const c_void, frame: *const VideoFrame) -> bool>,
    /// Callback is called when the audio frame is updated. The audio frame
    /// format is fixed to PCM. Be careful not to call blocking methods
    /// inside the callback, which will seriously slow down the
    /// encoding and decoding pipeline.
    ///
    /// Pulse-code modulation
    ///
    /// Pulse-code modulation (PCM) is a method used to digitally represent
    /// analog signals. It is the standard form of digital audio in
    /// computers, compact discs, digital telephony and other digital audio
    /// applications. In a PCM stream, the amplitude of the analog signal is
    /// sampled at uniform intervals, and each sample is quantized to the
    /// nearest value within a range of digital steps.
    ///
    /// Linear pulse-code modulation (LPCM) is a specific type of PCM in
    /// which the quantization levels are linearly uniform. This is
    /// in contrast to PCM encodings in which quantization levels
    /// vary as a function of amplitude (as with the A-law algorithm
    /// or the μ-law algorithm). Though PCM is a more general term,
    /// it is often used to describe data encoded as LPCM.
    ///
    /// A PCM stream has two basic properties that determine the stream's
    /// fidelity to the original analog signal: the sampling rate, which is
    /// the number of times per second that samples are taken; and the bit
    /// depth, which determines the number of possible digital values that
    /// can be used to represent each sample.
    pub(crate) audio: Option<extern "C" fn(ctx: *const c_void, frame: *const AudioFrame) -> bool>,
    /// Callback when the sender is closed. This may be because the external
    /// side actively calls the close, or the audio and video packets cannot
    /// be sent (the network is disconnected), etc.
    pub(crate) close: Option<extern "C" fn(ctx: *const c_void)>,
    pub(crate) ctx: *const c_void,
}

unsafe impl Send for RawAVFrameStream {}
unsafe impl Sync for RawAVFrameStream {}

impl AVFrameStream for RawAVFrameStream {}

impl AVFrameSink for RawAVFrameStream {
    fn audio(&self, frame: &AudioFrame) -> bool {
        if let Some(callback) = &self.audio {
            callback(self.ctx, frame)
        } else {
            true
        }
    }

    fn video(&self, frame: &VideoFrame) -> bool {
        if let Some(callback) = &self.video {
            callback(self.ctx, frame)
        } else {
            true
        }
    }
}

impl AVFrameObserver for RawAVFrameStream {
    fn close(&self) {
        if let Some(callback) = &self.close {
            callback(self.ctx);

            log::info!("extern api: call close callback");
        }
    }
}
