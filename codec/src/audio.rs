use crate::codec::{set_option, set_str_option};

use std::{ffi::c_int, ptr::null_mut};

use mirror_common::{c_str, frame::AudioFrame};
use mirror_ffmpeg_sys::*;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum AudioDecoderError {
    #[error("not found audio av coec")]
    NotFoundAVCodec,
    #[error("failed to alloc av context")]
    AllocAVContextError,
    #[error("failed to open av codec")]
    OpenAVCodecError,
    #[error("failed to init av parser context")]
    InitAVCodecParserContextError,
    #[error("failed to alloc av packet")]
    AllocAVPacketError,
    #[error("parser parse packet failed")]
    ParsePacketError,
    #[error("send av packet to codec failed")]
    SendPacketToAVCodecError,
    #[error("failed to alloc av frame")]
    AllocAVFrameError,
}

pub struct AudioDecoder {
    context: *mut AVCodecContext,
    parser: *mut AVCodecParserContext,
    packet: *mut AVPacket,
    av_frame: *mut AVFrame,
    frame: AudioFrame,
}

unsafe impl Sync for AudioDecoder {}
unsafe impl Send for AudioDecoder {}

impl AudioDecoder {
    pub fn new() -> Result<Self, AudioDecoderError> {
        let codec = unsafe { avcodec_find_decoder_by_name(c_str!("libopus")) };
        if codec.is_null() {
            return Err(AudioDecoderError::NotFoundAVCodec);
        }

        let mut this = Self {
            context: null_mut(),
            parser: null_mut(),
            packet: null_mut(),
            av_frame: null_mut(),
            frame: AudioFrame::default(),
        };

        this.context = unsafe { avcodec_alloc_context3(codec) };
        if this.context.is_null() {
            return Err(AudioDecoderError::AllocAVContextError);
        }

        let ch_layout = AVChannelLayout {
            order: AVChannelOrder::AV_CHANNEL_ORDER_NATIVE,
            nb_channels: 1,
            u: AVChannelLayout__bindgen_ty_1 {
                mask: AV_CH_LAYOUT_MONO,
            },
            opaque: null_mut(),
        };

        let context_mut = unsafe { &mut *this.context };
        context_mut.thread_count = 4;
        context_mut.thread_type = FF_THREAD_SLICE as i32;
        context_mut.request_sample_fmt = AVSampleFormat::AV_SAMPLE_FMT_S16;
        context_mut.ch_layout = ch_layout;
        context_mut.flags |= AV_CODEC_FLAG_LOW_DELAY as i32 | AVFMT_FLAG_NOBUFFER as i32;
        context_mut.flags2 |= AV_CODEC_FLAG2_FAST as i32;

        if unsafe { avcodec_open2(this.context, codec, null_mut()) } != 0 {
            return Err(AudioDecoderError::OpenAVCodecError);
        }

        if unsafe { avcodec_is_open(this.context) } == 0 {
            return Err(AudioDecoderError::OpenAVCodecError);
        }

        this.parser = unsafe { av_parser_init({ &*codec }.id as i32) };
        if this.parser.is_null() {
            return Err(AudioDecoderError::InitAVCodecParserContextError);
        }

        this.packet = unsafe { av_packet_alloc() };
        if this.packet.is_null() {
            return Err(AudioDecoderError::AllocAVPacketError);
        }

        this.av_frame = unsafe { av_frame_alloc() };
        if this.av_frame.is_null() {
            return Err(AudioDecoderError::AllocAVFrameError);
        }

        Ok(this)
    }

    pub fn decode(&mut self, mut buf: &[u8], pts: u64) -> Result<(), AudioDecoderError> {
        if buf.is_empty() {
            return Ok(());
        }

        let mut size = buf.len();
        while size > 0 {
            let packet = unsafe { &mut *self.packet };
            let len = unsafe {
                av_parser_parse2(
                    self.parser,
                    self.context,
                    &mut packet.data,
                    &mut packet.size,
                    buf.as_ptr(),
                    buf.len() as c_int,
                    pts as i64,
                    pts as i64,
                    0,
                )
            };

            // When parsing the code stream, an abnormal return code appears and processing
            // should not be continued.
            if len < 0 {
                return Err(AudioDecoderError::ParsePacketError);
            }

            let len = len as usize;
            buf = &buf[len..];
            size -= len;

            // One or more cells have been parsed.
            if packet.size > 0 {
                if unsafe { avcodec_send_packet(self.context, self.packet) } != 0 {
                    return Err(AudioDecoderError::SendPacketToAVCodecError);
                }
            }
        }

        Ok(())
    }

    pub fn read<'a>(&'a mut self) -> Option<&'a AudioFrame> {
        if !self.av_frame.is_null() {
            unsafe {
                av_frame_free(&mut self.av_frame);
            }
        }

        self.av_frame = unsafe { av_frame_alloc() };
        if self.av_frame.is_null() {
            return None;
        }

        if unsafe { avcodec_receive_frame(self.context, self.av_frame) } != 0 {
            return None;
        }

        let frame = unsafe { &*self.av_frame };
        self.frame.sample_rate = frame.sample_rate as u32;
        self.frame.frames = frame.nb_samples as u32;
        self.frame.data = frame.data[0] as *const _;

        Some(&self.frame)
    }
}

impl Drop for AudioDecoder {
    fn drop(&mut self) {
        if !self.packet.is_null() {
            unsafe {
                av_packet_free(&mut self.packet);
            }
        }

        if !self.parser.is_null() {
            unsafe {
                av_parser_close(self.parser);
            }
        }

        if !self.context.is_null() {
            unsafe {
                avcodec_free_context(&mut self.context);
            }
        }

        if !self.av_frame.is_null() {
            unsafe {
                av_frame_free(&mut self.av_frame);
            }
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct AudioEncoderSettings {
    pub bit_rate: u64,
    pub sample_rate: u64,
}

#[derive(Error, Debug)]
pub enum AudioEncoderError {
    #[error("not found audio av coec")]
    NotFoundAVCodec,
    #[error("failed to alloc av context")]
    AllocAVContextError,
    #[error("failed to open av codec")]
    OpenAVCodecError,
    #[error("failed to alloc av packet")]
    AllocAVPacketError,
    #[error("failed to alloc av frame")]
    AllocAVFrameError,
    #[error("send frame to codec failed")]
    EncodeFrameError,
}

pub struct AudioEncoder {
    context: *mut AVCodecContext,
    packet: *mut AVPacket,
    frame: *mut AVFrame,
    pts: i64,
}

unsafe impl Sync for AudioEncoder {}
unsafe impl Send for AudioEncoder {}

impl AudioEncoder {
    pub fn new(options: AudioEncoderSettings) -> Result<Self, AudioEncoderError> {
        let codec = unsafe { avcodec_find_encoder_by_name(c_str!("libopus")) };
        if codec.is_null() {
            return Err(AudioEncoderError::NotFoundAVCodec);
        }

        let mut this = Self {
            context: null_mut(),
            packet: null_mut(),
            frame: null_mut(),
            pts: 0,
        };

        this.context = unsafe { avcodec_alloc_context3(codec) };
        if this.context.is_null() {
            return Err(AudioEncoderError::AllocAVContextError);
        }

        let context_mut = unsafe { &mut *this.context };
        let ch_layout = AVChannelLayout {
            order: AVChannelOrder::AV_CHANNEL_ORDER_NATIVE,
            nb_channels: 1,
            u: AVChannelLayout__bindgen_ty_1 {
                mask: AV_CH_LAYOUT_MONO,
            },
            opaque: null_mut(),
        };

        context_mut.thread_count = 4;
        context_mut.thread_type = FF_THREAD_SLICE as i32;
        context_mut.sample_fmt = AVSampleFormat::AV_SAMPLE_FMT_S16;
        context_mut.ch_layout = ch_layout;
        context_mut.flags |= AV_CODEC_FLAG_LOW_DELAY as i32;
        context_mut.flags2 |= AV_CODEC_FLAG2_FAST as i32;

        context_mut.bit_rate = options.bit_rate as i64;
        context_mut.sample_rate = options.sample_rate as i32;
        context_mut.time_base = unsafe { av_make_q(1, options.sample_rate as i32) };

        // Forces opus to be encoded in units of 100 milliseconds.
        set_str_option(context_mut, "frame_duration", "100");
        set_option(context_mut, "application", 2051);

        if unsafe { avcodec_open2(this.context, codec, null_mut()) } != 0 {
            return Err(AudioEncoderError::OpenAVCodecError);
        }

        if unsafe { avcodec_is_open(this.context) } == 0 {
            return Err(AudioEncoderError::OpenAVCodecError);
        }

        this.packet = unsafe { av_packet_alloc() };
        if this.packet.is_null() {
            return Err(AudioEncoderError::AllocAVPacketError);
        }

        this.frame = unsafe { av_frame_alloc() };
        if this.frame.is_null() {
            return Err(AudioEncoderError::AllocAVFrameError);
        }

        Ok(this)
    }

    pub fn update(&mut self, frame: &AudioFrame) -> bool {
        let av_frame = unsafe { &mut *self.frame };
        let context_ref = unsafe { &*self.context };

        av_frame.nb_samples = frame.frames as i32;
        av_frame.format = context_ref.sample_fmt as i32;
        av_frame.ch_layout = context_ref.ch_layout;

        if unsafe { av_frame_get_buffer(self.frame, 0) } != 0 {
            return false;
        }

        unsafe {
            av_samples_fill_arrays(
                av_frame.data.as_mut_ptr(),
                av_frame.linesize.as_mut_ptr(),
                frame.data as *const _,
                1,
                frame.frames as i32,
                context_ref.sample_fmt,
                0,
            );
        }

        av_frame.pts = self.pts;
        self.pts += context_ref.sample_rate as i64 / 10;

        true
    }

    pub fn encode(&mut self) -> Result<(), AudioEncoderError> {
        if unsafe { avcodec_send_frame(self.context, self.frame) } != 0 {
            return Err(AudioEncoderError::EncodeFrameError);
        }

        unsafe {
            av_frame_unref(self.frame);
        }

        Ok(())
    }

    pub fn read<'a>(&'a mut self) -> Option<(&'a [u8], i32, u64)> {
        if unsafe { avcodec_receive_packet(self.context, self.packet) } != 0 {
            return None;
        }

        let packet_ref = unsafe { &*self.packet };
        Some((
            unsafe { std::slice::from_raw_parts(packet_ref.data, packet_ref.size as usize) },
            packet_ref.flags,
            packet_ref.pts as u64,
        ))
    }
}

impl Drop for AudioEncoder {
    fn drop(&mut self) {
        if !self.packet.is_null() {
            unsafe {
                av_packet_free(&mut self.packet);
            }
        }

        if !self.context.is_null() {
            unsafe {
                avcodec_free_context(&mut self.context);
            }
        }

        if !self.frame.is_null() {
            unsafe {
                av_frame_free(&mut self.frame);
            }
        }
    }
}

/// Header Packets
///
///    An Ogg Opus logical stream contains exactly two mandatory header
///    packets: an identification header and a comment header.
///
/// 5.1.  Identification Header
///
///       0                   1                   2                   3
///       0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1
///      +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
///      |      'O'      |      'p'      |      'u'      |      's'      |
///      +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
///      |      'H'      |      'e'      |      'a'      |      'd'      |
///      +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
///      |  Version = 1  | Channel Count |           Pre-skip            |
///      +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
///      |                     Input Sample Rate (Hz)                    |
///      +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
///      |   Output Gain (Q7.8 in dB)    | Mapping Family|               |
///      +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+               :
///      |                                                               |
///      :               Optional Channel Mapping Table...               :
///      |                                                               |
///      +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
///
///                         Figure 2: ID Header Packet
///
///    The fields in the identification (ID) header have the following
///    meaning:
///
///    1. Magic Signature:
///
///        This is an 8-octet (64-bit) field that allows codec
///        identification and is human readable.  It contains, in order, the
///        magic numbers:
///
///           0x4F 'O'
///
///           0x70 'p'
///
///           0x75 'u'
///
/// Terriberry, et al.           Standards Track                   [Page 12]
///
/// RFC 7845                        Ogg Opus                      April 2016
///
///
///           0x73 's'
///
///           0x48 'H'
///
///           0x65 'e'
///
///           0x61 'a'
///
///           0x64 'd'
///
///        Starting with "Op" helps distinguish it from audio data packets,
///        as this is an invalid TOC sequence.
///
///    2. Version (8 bits, unsigned):
///
///        The version number MUST always be '1' for this version of the
///        encapsulation specification.  Implementations SHOULD treat
///        streams where the upper four bits of the version number match
///        that of a recognized specification as backwards compatible with
///        that specification.  That is, the version number can be split
///        into "major" and "minor" version sub-fields, with changes to the
///        minor sub-field (in the lower four bits) signaling compatible
///        changes.  For example, an implementation of this specification
///        SHOULD accept any stream with a version number of '15' or less,
///        and SHOULD assume any stream with a version number '16' or
///        greater is incompatible.  The initial version '1' was chosen to
///        keep implementations from relying on this octet as a null
///        terminator for the "OpusHead" string.
///
///    3. Output Channel Count 'C' (8 bits, unsigned):
///
///        This is the number of output channels.  This might be different
///        than the number of encoded channels, which can change on a
///        packet-by-packet basis.  This value MUST NOT be zero.  The
///        maximum allowable value depends on the channel mapping family,
///        and might be as large as 255.  See Section 5.1.1 for details.
///
///    4. Pre-skip (16 bits, unsigned, little endian):
///
///        This is the number of samples (at 48 kHz) to discard from the
///        decoder output when starting playback, and also the number to
///        subtract from a page's granule position to calculate its PCM
///        sample position.  When cropping the beginning of existing Ogg
///        Opus streams, a pre-skip of at least 3,840 samples (80 ms) is
///        RECOMMENDED to ensure complete convergence in the decoder.
///
///
/// Terriberry, et al.           Standards Track                   [Page 13]
///
/// RFC 7845                        Ogg Opus                      April 2016
///
///
///    5. Input Sample Rate (32 bits, unsigned, little endian):
///
///        This is the sample rate of the original input (before encoding),
///        in Hz.  This field is _not_ the sample rate to use for playback
///        of the encoded data.
///
///        Opus can switch between internal audio bandwidths of 4, 6, 8, 12,
///        and 20 kHz.  Each packet in the stream can have a different audio
///        bandwidth.  Regardless of the audio bandwidth, the reference
///        decoder supports decoding any stream at a sample rate of 8, 12,
///        16, 24, or 48 kHz.  The original sample rate of the audio passed
///        to the encoder is not preserved by the lossy compression.
///
///        An Ogg Opus player SHOULD select the playback sample rate
///        according to the following procedure:
///
///        1. If the hardware supports 48 kHz playback, decode at 48 kHz.
///
///        2. Otherwise, if the hardware's highest available sample rate is a
///           supported rate, decode at this sample rate.
///
///        3. Otherwise, if the hardware's highest available sample rate is less
///           than 48 kHz, decode at the next higher Opus supported rate above
///           the highest available hardware rate and resample.
///
///        4. Otherwise, decode at 48 kHz and resample.
///
///        However, the 'input sample rate' field allows the muxer to pass
///        the sample rate of the original input stream as metadata.  This
///        is useful when the user requires the output sample rate to match
///        the input sample rate.  For example, when not playing the output,
///        an implementation writing PCM format samples to disk might choose
///        to resample the audio back to the original input sample rate to
///        reduce surprise to the user, who might reasonably expect to get
///        back a file with the same sample rate.
///
///        A value of zero indicates "unspecified".  Muxers SHOULD write the
///        actual input sample rate or zero, but implementations that do
///        something with this field SHOULD take care to behave sanely if
///        given crazy values (e.g., do not actually upsample the output to
///        10 MHz if requested).  Implementations SHOULD support input
///        sample rates between 8 kHz and 192 kHz (inclusive).  Rates
///        outside this range MAY be ignored by falling back to the default
///        rate of 48 kHz instead.
///
/// Terriberry, et al.           Standards Track                   [Page 14]
///
/// RFC 7845                        Ogg Opus                      April 2016
///
///
///    6. Output Gain (16 bits, signed, little endian):
///
///        This is a gain to be applied when decoding.  It is 20*log10 of
///        the factor by which to scale the decoder output to achieve the
///        desired playback volume, stored in a 16-bit, signed, two's
///        complement fixed-point value with 8 fractional bits (i.e.,
///        Q7.8 [Q-NOTATION]).
///
///        To apply the gain, an implementation could use the following:
///
///                  sample *= pow(10, output_gain/(20.0*256))
///
///        where 'output_gain' is the raw 16-bit value from the header.
///
///        Players and media frameworks SHOULD apply it by default.  If a
///        player chooses to apply any volume adjustment or gain
///        modification, such as the R128_TRACK_GAIN (see Section 5.2), the
///        adjustment MUST be applied in addition to this output gain in
///        order to achieve playback at the normalized volume.
///
///        A muxer SHOULD set this field to zero, and instead apply any gain
///        prior to encoding, when this is possible and does not conflict
///        with the user's wishes.  A nonzero output gain indicates the gain
///        was adjusted after encoding, or that a user wished to adjust the
///        gain for playback while preserving the ability to recover the
///        original signal amplitude.
///
///        Although the output gain has enormous range (+/- 128 dB, enough
///        to amplify inaudible sounds to the threshold of physical pain),
///        most applications can only reasonably use a small portion of this
///        range around zero.  The large range serves in part to ensure that
///        gain can always be losslessly transferred between OpusHead and
///        R128 gain tags (see below) without saturating.
///
///    7. Channel Mapping Family (8 bits, unsigned):
///
///        This octet indicates the order and semantic meaning of the output
///        channels.
///
///        Each currently specified value of this octet indicates a mapping
///        family, which defines a set of allowed channel counts, and the
///        ordered set of channel names for each allowed channel count.  The
///        details are described in Section 5.1.1.
///
///    8. Channel Mapping Table:
///
///        This table defines the mapping from encoded streams to output
///        channels.  Its contents are specified in Section 5.1.1.
#[inline]
#[rustfmt::skip]
pub fn create_opus_identification_header(channel: u8, sample_rate: u32) -> [u8; 83] {
    let sample_rate = sample_rate.to_le_bytes();

    [
        // AOPUSHDR
        0x41, 0x4f, 0x50, 0x55, 0x53, 0x48, 0x44, 0x52,
        0x13, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,

        // OpusHead
        0x4f, 0x70, 0x75, 0x73,  0x48, 0x65, 0x61, 0x64,

        // Version
        0x01,

        // Channel Count
        channel,

        // Pre skip
        0x00, 0x00,

        // Input Sample Rate (Hz), eg: 48000
        sample_rate[0],
        sample_rate[1],
        sample_rate[2],
        sample_rate[3],

        // Output Gain (Q7.8 in dB) 
        0x00, 0x00,

        // Mapping Family
        0x00,

        // AOPUSDLY
        0x41, 0x4f, 0x50, 0x55, 0x53, 0x44, 0x4c, 0x59,
        0x08, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 
        0xa0, 0x2e, 0x63, 0x00, 0x00, 0x00, 0x00, 0x00,
        
        // AOPUSPRL
        0x41, 0x4f, 0x50, 0x55, 0x53, 0x50, 0x52, 0x4c, 
        0x08, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 
        0x00, 0xb4, 0xc4, 0x04, 0x00, 0x00, 0x00, 0x00,
    ]
}
