use crate::{Error, RawPacket};

use std::{
    ffi::{c_char, CString},
    os::raw::c_void,
};

use frame::AudioFrame;
use utils::strings::Strings;

extern "C" {
    fn codec_create_audio_decoder(codec_name: *const c_char) -> *const c_void;
    fn codec_audio_decoder_send_packet(codec: *const c_void, packet: *const RawPacket) -> bool;
    fn codec_audio_decoder_read_frame(codec: *const c_void) -> *const AudioFrame;
    fn codec_release_audio_decoder(codec: *const c_void);
    fn codec_create_audio_encoder(settings: *const RawAudioEncoderSettings) -> *const c_void;
    fn codec_audio_encoder_copy_frame(codec: *const c_void, frame: *const AudioFrame) -> bool;
    fn codec_audio_encoder_send_frame(codec: *const c_void) -> bool;
    fn codec_audio_encoder_read_packet(codec: *const c_void) -> *const RawPacket;
    fn codec_unref_audio_encoder_packet(codec: *const c_void);
    fn codec_release_audio_encoder(codec: *const c_void);
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
        // ...
        0x13, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        // Opus
        0x4f, 0x70, 0x75, 0x73,
        // Head
        0x48, 0x65, 0x61, 0x64,
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
        // ...
        0x08, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 
        0xa0, 0x2e, 0x63, 0x00, 0x00, 0x00, 0x00, 0x00,
        // AOPUSPRL
        0x41, 0x4f, 0x50, 0x55, 0x53, 0x50, 0x52, 0x4c, 
        // ...
        0x08, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 
        0x00, 0xb4, 0xc4, 0x04, 0x00, 0x00, 0x00, 0x00,
    ]
}

pub struct AudioDecoder(*const c_void);

unsafe impl Send for AudioDecoder {}
unsafe impl Sync for AudioDecoder {}

impl AudioDecoder {
    /// Initialize the AVCodecContext to use the given AVCodec.
    pub fn new(codec: &str) -> Result<Self, Error> {
        log::info!("create AudioDecoder: codec name={:?}", codec);

        let codec = unsafe { codec_create_audio_decoder(Strings::from(codec).as_ptr()) };
        if !codec.is_null() {
            Ok(Self(codec))
        } else {
            Err(Error::AudioDecoder)
        }
    }

    /// Supply raw packet data as input to a decoder.
    pub fn decode(&mut self, data: &[u8], flags: i32, timestamp: u64) -> bool {
        unsafe {
            codec_audio_decoder_send_packet(
                self.0,
                &RawPacket {
                    buffer: data.as_ptr(),
                    len: data.len(),
                    timestamp,
                    flags,
                },
            )
        }
    }

    /// Return decoded output data from a decoder or encoder (when the
    /// AV_CODEC_FLAG_RECON_FRAME flag is used).
    pub fn read(&mut self) -> Option<&AudioFrame> {
        let frame = unsafe { codec_audio_decoder_read_frame(self.0) };
        if !frame.is_null() {
            Some(unsafe { &*frame })
        } else {
            None
        }
    }
}

impl Drop for AudioDecoder {
    fn drop(&mut self) {
        log::info!("close AudioDecoder");

        unsafe { codec_release_audio_decoder(self.0) }
    }
}

#[repr(C)]
pub struct RawAudioEncoderSettings {
    pub codec: *const c_char,
    pub bit_rate: u64,
    pub sample_rate: u64,
}

impl Drop for RawAudioEncoderSettings {
    fn drop(&mut self) {
        drop(unsafe { CString::from_raw(self.codec as *mut _) })
    }
}

#[derive(Debug, Clone)]
pub struct AudioEncoderSettings {
    pub codec: String,
    pub bit_rate: u64,
    pub sample_rate: u64,
}

impl AudioEncoderSettings {
    fn as_raw(&self) -> RawAudioEncoderSettings {
        RawAudioEncoderSettings {
            codec: CString::new(self.codec.as_str()).unwrap().into_raw(),
            sample_rate: self.sample_rate,
            bit_rate: self.bit_rate,
        }
    }
}

#[repr(C)]
pub struct AudioEncodePacket<'a> {
    codec: *const c_void,
    pub buffer: &'a [u8],
    pub flags: i32,
    pub timestamp: u64,
}

impl Drop for AudioEncodePacket<'_> {
    fn drop(&mut self) {
        unsafe { codec_unref_audio_encoder_packet(self.codec) }
    }
}

impl<'a> AudioEncodePacket<'a> {
    fn from_raw(codec: *const c_void, ptr: *const RawPacket) -> Self {
        let raw = unsafe { &*ptr };
        Self {
            buffer: unsafe { std::slice::from_raw_parts(raw.buffer, raw.len) },
            timestamp: raw.timestamp,
            flags: raw.flags,
            codec,
        }
    }
}

pub struct AudioEncoder(*const c_void);

unsafe impl Send for AudioEncoder {}
unsafe impl Sync for AudioEncoder {}

impl AudioEncoder {
    /// Initialize the AVCodecContext to use the given AVCodec.
    pub fn new(settings: &AudioEncoderSettings) -> Result<Self, Error> {
        log::info!("create AudioEncoder: settings={:?}", settings);

        let settings = settings.as_raw();
        let codec = unsafe { codec_create_audio_encoder(&settings) };
        if !codec.is_null() {
            Ok(Self(codec))
        } else {
            Err(Error::AudioEncoder)
        }
    }

    pub fn send_frame(&mut self, frame: &AudioFrame) -> bool {
        unsafe { codec_audio_encoder_copy_frame(self.0, frame) }
    }

    /// Supply a raw video or audio frame to the encoder.
    pub fn encode(&mut self) -> bool {
        unsafe { codec_audio_encoder_send_frame(self.0) }
    }

    /// Read encoded data from the encoder.
    pub fn read(&mut self) -> Option<AudioEncodePacket> {
        let packet = unsafe { codec_audio_encoder_read_packet(self.0) };
        if !packet.is_null() {
            Some(AudioEncodePacket::from_raw(self.0, packet))
        } else {
            None
        }
    }
}

impl Drop for AudioEncoder {
    fn drop(&mut self) {
        log::info!("close AudioEncoder");

        unsafe { codec_release_audio_encoder(self.0) }
    }
}
