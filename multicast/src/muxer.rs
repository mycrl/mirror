use bytes::{BufMut, BytesMut};

/// Packet repackaging.
///
/// For UDP packets, the packet sequence number is reappended here.
pub struct Muxer {
    buffer: BytesMut,
    seq_number: u16,
    mtu: usize,
}

impl Muxer {
    /// Create a muxer and you need to specify the maximum package unit size.
    pub fn new(mtu: usize) -> Self {
        Self {
            buffer: BytesMut::with_capacity(mtu),
            seq_number: 0,
            mtu,
        }
    }

    /// Gets the maximum length of the packet.
    ///
    /// Each Ethernet frame has a minimum size of 64 bytes and a maximum of 1518
    /// bytes.
    ///
    /// Excluding the 18 bytes at the beginning and end of the link layer, the
    /// data area at the link layer ranges from 46 to 1500 bytes.
    ///
    /// Then the data area of the link layer, i.e., the MTU (Maximum
    /// Transmission Unit) is 1500 bytes.
    ///
    /// In fact, this 1500 bytes is the length limit for IP datagrams at the
    /// network layer.
    ///
    /// Because the first part of an IP datagram is 20 bytes, the maximum length
    /// of the IP datagram data area is 1480 bytes. This 1,480 bytes is used for
    /// TCP packets from TCP or UDP packets from UDP.
    ///
    /// Excluding the 8 bytes of the UDP packet header, the maximum length of
    /// the data area of a UDP datagram is 1472 bytes.
    ///
    /// In a LAN environment, it is recommended to limit the UDP data to less
    /// than 1472 bytes.
    pub fn max_payload_size(&self) -> usize {
        self.mtu - 30
    }

    /// Processes the packets after mixing them and returns the processed
    /// packets.
    pub fn mux(&mut self, buf: &[u8]) -> &[u8] {
        assert!(!buf.is_empty());

        // Write the sequence number and packet contents.
        self.buffer.clear();
        self.buffer.put_u16(self.seq_number);
        self.buffer.put(buf);

        // Increment the sequence number, starting at 0 if it overflows.
        self.seq_number = if self.seq_number < u16::MAX {
            self.seq_number + 1
        } else {
            0
        };

        &self.buffer[..]
    }
}
