mod receiver;
mod sender;

use std::{
    ffi::CStr,
    net::{IpAddr, SocketAddr},
    sync::Arc,
};

pub use receiver::RtpReceiver;
pub use sender::RtpSender;

#[derive(Debug, Clone, Copy)]
pub struct RtpConfig {
    pub bind: SocketAddr,
    pub dest: SocketAddr,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RtpErrorKind {
    NotSupportIPV6,
    UnableToCreateRTP,
    UnableToSendPacket,
    Unreadable,
    NoMorePackets,
}

#[derive(Debug, Clone)]
pub struct RtpError {
    pub kind: RtpErrorKind,
    pub message: Option<String>,
}

impl RtpError {
    fn error<T>(kind: RtpErrorKind) -> Result<T, Self> {
        let mut msg = [0; 255];
        unsafe {
            api::get_latest_error(msg.as_mut_ptr());
        }

        Err(Self {
            kind,
            message: unsafe { CStr::from_ptr(msg.as_ptr()) }
                .to_str()
                .map(|s| s.to_string())
                .ok(),
        })
    }
}

impl std::error::Error for RtpError {}

impl std::fmt::Display for RtpError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "kind = {:?}, message = {:?}", self.kind, self.message)
    }
}

/// Wrapper for rtp sessions, hosting the autorelease c++ class.
///
/// The RTPSession class pointer is passable across threads and is internally
/// thread-safe, so there is no problem with this class implementing Sync and
/// Send.
pub(crate) struct Rtp(api::RtpRef);

unsafe impl Send for Rtp {}
unsafe impl Sync for Rtp {}

impl Rtp {
    fn new(raw: api::RtpRef) -> Self {
        Self(raw)
    }

    /// Returns a pointer to the original C++ RTPSession class.
    fn as_raw(&self) -> api::RtpRef {
        self.0
    }
}

impl Drop for Rtp {
    fn drop(&mut self) {
        unsafe { api::close_rtp(self.0) }
    }
}

/// The rtp internal packet reference automatically manages the release of
/// internal memory.
pub struct Packet {
    pkt: api::PacketRef,
    rtp: Arc<Rtp>,
}

impl Packet {
    fn new(rtp: Arc<Rtp>, pkt: api::PacketRef) -> Self {
        Self { rtp, pkt }
    }

    /// Getting the raw load inside an rtp packet is less recommended for
    /// frequent calls.
    pub fn as_bytes(&self) -> &[u8] {
        let packet = unsafe { &*api::get_packet_ref(self.rtp.as_raw(), self.pkt) };
        unsafe { std::slice::from_raw_parts(packet.buf, packet.size) }
    }
}

impl Drop for Packet {
    fn drop(&mut self) {
        unsafe { api::unref_packet(self.rtp.as_raw(), self.pkt) }
    }
}

pub(crate) fn inetv4_addr(addr: &SocketAddr) -> Result<u32, RtpError> {
    if let IpAddr::V4(ip) = addr.ip() {
        Ok(u32::from_be_bytes(ip.octets()))
    } else {
        RtpError::error(RtpErrorKind::NotSupportIPV6)
    }
}

mod api {
    use std::ffi::{c_char, c_void};

    pub type RtpRef = *const c_void;
    pub type PacketRef = *const c_void;

    /// The RTPPacket class can be used to parse a RTPRawPacket instance if it
    /// represents RTP data. The class can also be used to create a new RTP
    /// packet according to the parameters specified by the user.
    #[repr(C)]
    pub struct Packet {
        pub buf: *const u8,
        pub size: usize,
    }

    extern "C" {
        pub fn get_latest_error(msg: *mut c_char);
        /// Sends a BYE packet and leaves the session. At most a time
        /// maxwaittime will be waited to send the BYE packet. If this time
        /// expires, the session will be left without sending a BYE packet. The
        /// BYE packet will contain as reason for leaving reason with length
        /// reasonlength.
        pub fn close_rtp(rtp: RtpRef);
        /// To use RTP, you'll have to create an RTPSession object. The
        /// constructor accepts two parameter, an instance of an RTPRandom
        /// object, and an instance of an RTPMemoryManager object.
        pub fn create_sender(bind_ip: u32, bind_port: u16, dest_ip: u32, dest_port: u16) -> RtpRef;
        pub fn create_receiver(
            bind_ip: u32,
            bind_port: u16,
            dest_ip: u32,
            dest_port: u16,
        ) -> RtpRef;
        /// Sends the RTP packet with payload data which has length len. The
        /// used payload type, marker and timestamp increment will be those that
        /// have been set using the SetDefault member functions.
        pub fn send_packet(rtp: RtpRef, pkt: *const Packet) -> bool;
        /// The BeginDataAccess function makes sure that the poll thread won't
        /// access the source table at the same time that you're using it. When
        /// the EndDataAccess is called, the lock on the source table is freed
        /// again.
        pub fn lock_poll_thread(rtp: RtpRef) -> bool;
        pub fn unlock_poll_thread(rtp: RtpRef) -> bool;
        /// Sets the current source to be the first source in the table which
        /// has RTPPacket instances that we haven't extracted yet. If no such
        /// member was found, the function returns false, otherwise it returns
        /// true.
        pub fn goto_first_source(rtp: RtpRef) -> bool;
        /// Sets the current source to be the next source in the table. If we're
        /// already at the last source, the function returns false, otherwise it
        /// returns true.
        pub fn goto_next_source(rtp: RtpRef) -> bool;
        /// Extracts the next packet from the received packets queue of the
        /// current participant, or NULL if no more packets are available. When
        /// the packet is no longer needed, its memory should be freed using the
        /// DeletePacket member function.
        pub fn get_next_packet(rtp: RtpRef) -> PacketRef;
        pub fn get_packet_ref(rtp: RtpRef, pkt: RtpRef) -> *const Packet;
        /// Frees the memory used by p.
        pub fn unref_packet(rtp: RtpRef, pkt: RtpRef);
    }
}
