use crate::{api, inetv4_addr, Rtp, RtpConfig, RtpError, RtpErrorKind};

/// The sender of rtp packets can send point-to-point as well as broadcast
/// packets.
pub struct RtpSender(Rtp);

impl RtpSender {
    pub const fn max_packet_size() -> usize {
        1300
    }

    /// To create a sender, you need to specify a destination for sending
    /// packets.
    pub fn new(cfg: RtpConfig) -> Result<Self, RtpError> {
        let ptr = unsafe {
            api::create_sender(
                inetv4_addr(&cfg.bind)?,
                cfg.bind.port(),
                inetv4_addr(&cfg.dest)?,
                cfg.dest.port(),
            )
        };

        if ptr.is_null() {
            RtpError::error(RtpErrorKind::UnableToCreateRTP)
        } else {
            Ok(Self(Rtp::new(ptr)))
        }
    }

    /// Send packets to the other end, it should be noted that there is a limit
    /// to the size of packets that can be sent in a single pass, here the limit
    /// is 1400 bytes.
    pub fn send(&self, buf: &[u8]) -> Result<(), RtpError> {
        assert!(buf.len() <= Self::max_packet_size());

        if !unsafe {
            api::send_packet(
                self.0.as_raw(),
                &api::Packet {
                    buf: buf.as_ptr(),
                    size: buf.len(),
                },
            )
        } {
            RtpError::error(RtpErrorKind::UnableToSendPacket)
        } else {
            Ok(())
        }
    }
}
