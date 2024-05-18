mod decode;
mod encode;

pub use decode::AudioDecoder;
pub use encode::{AudioEncodePacket, AudioEncoder, AudioEncoderSettings};

// [65, 79, 80, 85, 83, 72, 68, 82, 19, 0, 0, 0, 0, 0, 0, 0, 79, 112, 117, 115, 72, 101, 97, 100, 1, 1, 56, 1, 128, 187, 0, 0, 0, 0, 0, 65, 79, 80, 85, 83, 68, 76, 89, 8, 0, 0, 0, 0, 0, 0, 0, 160, 46, 99, 0, 0, 0, 0, 0, 65, 79, 80, 85, 83, 80, 82, 76, 8, 0, 0, 0, 0, 0, 0, 0, 0, 180, 196, 4, 0, 0, 0, 0]
