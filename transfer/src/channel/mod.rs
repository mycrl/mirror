//! use std::{net::SocketAddr, time::Duration};
//!
//! use srt::{Listener, Socket, SrtError};
//! use tokio::time::sleep;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), SrtError> {
//!     tokio::spawn(async {
//!         let mut server = Listener::bind(
//!             &"0.0.0.0:3000".parse::<SocketAddr>().unwrap()
//!         ).await?;
//!
//!         println!("start server, listening....");
//!
//!         while let Ok((socket, addr)) = server.accept().await {
//!             println!("server accept socket: addr={}", addr);
//!
//!             tokio::spawn(async move {
//!                 for _ in 0..5 {
//!                     sleep(Duration::from_secs(1)).await;
//!                     socket.send(&[0x01, 0x02]).await?;
//!                 }
//!
//!                 #[allow(unused)]
//!                 Ok::<(), SrtError>(())
//!             });
//!         }
//!
//!         Ok::<(), SrtError>(())
//!     });
//!
//!     sleep(Duration::from_secs(5)).await;
//!     println!("connecting...");
//!
//!     let mut buf = [0u8; 2048];
//!     let socket = Socket::connect(
//!         &"127.0.0.1:3000".parse::<SocketAddr>().unwrap()
//!     ).await?;
//!
//!     while let Ok(size) = socket.read(&mut buf).await {
//!         println!("socket recv: buf={:?}", &buf[..size]);
//!     }
//!
//!     Ok(())
//! }

mod server;
mod options;
mod socket;
mod fragments;

pub use self::{server::Server, options::Options, socket::Socket};

use std::ffi::{c_char, c_int, c_void, CStr};

use libc::sockaddr;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorKind {
    InvalidSock,
    BindError,
    ListenError,
    AcceptError,
    RecvError,
    SendError,
    ConnectError,
    GetStatsError,
    SetOptError,
}

#[derive(Debug, Clone)]
pub struct Error {
    pub kind: ErrorKind,
    pub message: Option<String>,
}

impl Error {
    fn error<T>(kind: ErrorKind) -> Result<T, Self> {
        Err(Self {
            kind,
            message: unsafe { CStr::from_ptr(srt_getlasterror_str()) }
                .to_str()
                .map(|s| s.to_string())
                .ok(),
        })
    }
}

impl std::error::Error for Error {}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "kind = {:?}, message = {:?}", self.kind, self.message)
    }
}

/// This function shall be called at the start of an application that uses
/// the SRT library. It provides all necessary platform-specific
/// initializations, sets up global data, and starts the SRT GC thread.
/// If this function isn't explicitly called, it will be called
/// automatically when creating the first socket. However, relying on
/// this behavior is strongly discouraged.
pub fn startup() -> bool {
    unsafe { srt_startup() != -1 }
}

/// This function cleans up all global SRT resources and shall be called
/// just before exiting the application that uses the SRT library. This
/// cleanup function will still be called from the C++ global
/// destructor, if not called by the application, although relying on
/// this behavior is strongly discouraged.
pub fn cleanup() {
    unsafe {
        srt_cleanup();
    }
}

pub type SRTSOCKET = i32;
pub const SRT_INVALID_SOCK: i32 = -1;

#[repr(C)]
#[allow(unused)]
#[allow(non_camel_case_types)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SRT_TRANSTYPE {
    SRTT_LIVE,
    SRTT_FILE,
    SRTT_INVALID,
}

#[repr(C)]
#[allow(unused)]
#[allow(non_camel_case_types)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SRT_SOCKOPT {
    SRTO_MSS = 0,
    SRTO_SNDSYN = 1,
    SRTO_RCVSYN = 2,
    SRTO_ISN = 3,
    SRTO_FC = 4,
    SRTO_SNDBUF = 5,
    SRTO_RCVBUF = 6,
    SRTO_LINGER = 7,
    SRTO_UDP_SNDBUF = 8,
    SRTO_UDP_RCVBUF = 9,
    SRTO_RENDEZVOUS = 12,
    SRTO_SNDTIMEO = 13,
    SRTO_RCVTIMEO = 14,
    SRTO_REUSEADDR = 15,
    SRTO_MAXBW = 16,
    SRTO_STATE = 17,
    SRTO_EVENT = 18,
    SRTO_SNDDATA = 19,
    SRTO_RCVDATA = 20,
    SRTO_SENDER = 21,
    SRTO_TSBPDMODE = 22,
    SRTO_LATENCY = 23,
    SRTO_INPUTBW = 24,
    SRTO_OHEADBW,
    SRTO_PASSPHRASE = 26,
    SRTO_PBKEYLEN,
    SRTO_KMSTATE,
    SRTO_IPTTL = 29,
    SRTO_IPTOS,
    SRTO_TLPKTDROP = 31,
    SRTO_SNDDROPDELAY = 32,
    SRTO_NAKREPORT = 33,
    SRTO_VERSION = 34,
    SRTO_PEERVERSION,
    SRTO_CONNTIMEO = 36,
    SRTO_DRIFTTRACER = 37,
    SRTO_MININPUTBW = 38,
    SRTO_SNDKMSTATE = 40,
    SRTO_RCVKMSTATE,
    SRTO_LOSSMAXTTL,
    SRTO_RCVLATENCY,
    SRTO_PEERLATENCY,
    SRTO_MINVERSION,
    SRTO_STREAMID,
    SRTO_CONGESTION,
    SRTO_MESSAGEAPI,
    SRTO_PAYLOADSIZE,
    SRTO_TRANSTYPE = 50,
    SRTO_KMREFRESHRATE,
    SRTO_KMPREANNOUNCE,
    SRTO_ENFORCEDENCRYPTION,
    SRTO_IPV6ONLY,
    SRTO_PEERIDLETIMEO,
    SRTO_BINDTODEVICE,
    SRTO_GROUPCONNECT,
    SRTO_GROUPMINSTABLETIMEO,
    SRTO_GROUPTYPE,
    SRTO_PACKETFILTER = 60,
    SRTO_RETRANSMITALGO = 61,
    SRTO_E_SIZE,
}

extern "C" {
    pub fn srt_getlasterror_str() -> *const c_char;
    /// This function shall be called at the start of an application that
    /// uses the SRT library. It provides all necessary
    /// platform-specific initializations, sets up global data, and
    /// starts the SRT GC thread. If this function isn't explicitly
    /// called, it will be called automatically when creating the
    /// first socket. However, relying on this behavior is strongly
    /// discouraged.
    pub fn srt_startup() -> c_int;
    /// This function cleans up all global SRT resources and shall be called
    /// just before exiting the application that uses the SRT library. This
    /// cleanup function will still be called from the C++ global
    /// destructor, if not called by the application, although relying on
    /// this behavior is strongly discouraged.
    pub fn srt_cleanup() -> c_int;
    /// Creates an SRT socket.
    ///
    /// Note that socket IDs always have the `SRTGROUP_MASK` bit clear.
    pub fn srt_create_socket() -> SRTSOCKET;
    /// Binds a socket to a local address and port. Binding specifies the
    /// local network interface and the UDP port number to be used
    /// for the socket. When the local address is a wildcard
    /// (`INADDR_ANY` for IPv4 or `in6addr_any` for IPv6), then it's
    /// bound to all interfaces.
    ///
    /// **IMPORTANT**: When you bind an IPv6 wildcard address, note that the
    /// `SRTO_IPV6ONLY` option must be set on the socket explicitly to 1 or
    /// 0 prior to calling this function. See
    /// [`SRTO_IPV6ONLY`](API-socket-options.md#SRTO_IPV6ONLY) for more
    /// details.
    ///
    /// Binding is necessary for every socket to be used for communication.
    /// If the socket is to be used to initiate a connection to a
    /// listener socket, which can be done, for example, by the
    /// [`srt_connect`](#srt_connect) function, the socket is bound
    /// implicitly to the wildcard address according to the IP family
    /// (`INADDR_ANY` for `AF_INET` or `in6addr_any` for `AF_INET6`) and
    /// port number 0. In all other cases, a socket must be bound explicitly
    /// by using the functionality of this function first.
    ///
    /// When the port number parameter is 0, then the effective port number
    /// will be system-allocated. To obtain this effective port
    /// number you can use [`srt_getsockname`](#srt_getsockname).
    ///
    /// This call is obligatory for a listening socket before calling
    /// [`srt_listen`](#srt_listen) and for rendezvous mode before calling
    /// [`srt_connect`](#srt_connect); otherwise it's optional. For a
    /// listening socket it defines the network interface and the port where
    /// the listener should expect a call request.
    ///
    /// In the case of rendezvous mode there are two parties that connect to
    /// one another. For every party there must be chosen a local
    /// binding endpoint (local address and port) to which they
    /// expect connection from the peer. Let's say, we have a Party
    /// 1 that selects an endpoint A and a Party 2 that selects an
    /// endpoint B. In this case the Party 1 binds the socket to the
    /// endpoint A and then connects to the endpoint B,
    /// and the Party 2 the other way around. Both sockets must be set
    /// [`SRTO_RENDEZVOUS`](API-socket-options.md#SRTO_RENDEZVOUS) to *true*
    /// to make this connection possible.
    ///
    /// For a connecting socket the call to `srt_bind` is optional, but can
    /// be used to set up the outgoing port for communication as
    /// well as the local interface through which it should reach
    /// out to the remote endpoint, should that be necessary.
    ///
    /// Whether binding is possible depends on some runtime conditions, in
    /// particular:
    ///
    /// * No socket in the system has been bound to this port ("free binding"),
    ///   or
    ///
    /// * A socket bound to this port is bound to a certain address, and this
    ///   binding is
    /// using a different non-wildcard address ("side binding"), or
    ///
    /// * A socket bound to this port is bound to a wildcard address for a
    ///   different IP
    /// version than the version requested for this binding ("side wildcard
    /// binding", see also `SRTO_IPV6ONLY` socket option).
    ///
    /// It is also possible to bind to the already busy port as long as the
    /// existing binding ("shared binding") is possessed by an SRT socket
    /// created in the same application, and:
    ///
    /// * Its binding address and UDP-related socket options match the socket to
    ///   be bound.
    /// * Its [`SRTO_REUSEADDR`](API-socket-options.md#SRTO_REUSEADDRS) is set
    ///   to *true* (default).
    ///
    /// If none of the free, side and shared binding options is currently
    /// possible, this function will fail. If the socket blocking the
    /// requested endpoint is an SRT socket in the current application, it
    /// will report the `SRT_EBINDCONFLICT` error, while if it was another
    /// socket in the system, or the problem was in the system in general,
    /// it will report `SRT_ESOCKFAIL`. Here is the table that shows
    /// possible situations:
    ///
    /// Where:
    ///
    /// * free: This binding can coexist with the requested binding.
    ///
    /// * blocked: This binding conflicts with the requested binding.
    ///
    /// * shareable: This binding can be shared with the requested binding if
    ///   it's compatible.
    ///
    /// * (ADDRESS) shareable, else free: this binding is shareable if the
    ///   existing binding address is
    /// equal to the requested ADDRESS. Otherwise it's free.
    ///
    /// If the binding is shareable, then the operation will succeed if the
    /// socket that currently occupies the binding has the `SRTO_REUSEADDR`
    /// option set to true (default) and all UDP settings are the same as in
    /// the current socket. Otherwise it will fail. Shared binding means
    /// sharing the underlying UDP socket and communication queues between
    /// SRT sockets. If all existing bindings on the same port are
    /// "free" then the requested binding will allocate a distinct
    /// UDP socket for this SRT socket ("side binding").
    ///
    /// **NOTE**: This function cannot be called on a socket group. If you
    /// need to have the group-member socket bound to the specified
    /// source address before connecting, use
    /// [`srt_connect_bind`](#srt_connect_bind) for that purpose or set the
    /// appropriate source address using [`srt_prepare_endpoint`](#
    /// srt_prepare_endpoint).
    ///
    /// **IMPORTANT information about IPv6**: If you are going to bind to
    /// the `in6addr_any` IPv6 wildcard address (known as `::`), the
    /// `SRTO_IPV6ONLY` option must be first set explicitly to 0 or
    /// 1, otherwise the binding will fail. In all other cases this
    /// option is meaningless. See `SRTO_IPV6ONLY` option for more
    /// information.
    pub fn srt_bind(s: SRTSOCKET, name: *const sockaddr, name_len: c_int) -> c_int;
    /// Closes the socket or group and frees all used resources. Note that
    /// underlying UDP sockets may be shared between sockets, so these are
    /// freed only with the last user closed.
    pub fn srt_close(s: SRTSOCKET) -> c_int;
    /// This sets up the listening state on a socket with a backlog setting
    /// that defines how many sockets may be allowed to wait until
    /// they are accepted (excessive connection requests are
    /// rejected in advance).
    ///
    /// The following important options may change the behavior of the
    /// listener socket and the [`srt_accept`](#srt_accept)
    /// function:
    ///
    /// * [`srt_listen_callback`](#srt_listen_callback) installs a user function
    ///   that will
    /// be called before [`srt_accept`](#srt_accept) can happen
    /// * [`SRTO_GROUPCONNECT`](API-socket-options.md#SRTO_GROUPCONNECT) option
    ///   allows
    /// the listener socket to accept group connections
    pub fn srt_listen(s: SRTSOCKET, backlog: c_int) -> c_int;
    pub fn srt_listen_callback(
        s: SRTSOCKET,
        hook_fn: extern "C" fn(
            opaque: *mut c_void,
            s: SRTSOCKET,
            hs_version: c_int,
            peeraddr: *const sockaddr,
            streamid: *const c_char,
        ) -> c_int,
        hook_opaque: *mut c_void,
    ) -> c_int;
    /// Connects a socket or a group to a remote party with a specified
    /// address and port.
    ///
    /// **Arguments**:
    ///
    /// * [`u`](#u): can be an SRT socket or SRT group, both freshly created and
    ///   not yet used for any connection, except possibly
    ///   [`srt_bind`](#srt_bind) on the socket
    /// * `name`: specification of the remote address and port
    /// * `namelen`: size of the object passed by `name`
    ///
    /// **NOTES:**
    ///
    /// 1. The socket used here may be [bound by `srt_bind`](#srt_bind) before
    ///    connecting,
    /// or binding and connection can be done in one function
    /// ([`srt_connect_bind`](#srt_connect_bind)), such that it uses a
    /// predefined network interface or local outgoing port. This is
    /// optional in the case of a caller-listener arrangement, but
    /// obligatory for a rendezvous arrangement. If not used, the
    /// binding will be done automatically to `INADDR_ANY` (which
    /// binds on all interfaces) and port 0 (which makes the system
    /// assign the port automatically).
    ///
    /// 2. This function is used for both connecting to the listening peer in a
    ///    caller-listener
    /// arrangement, and calling the peer in rendezvous mode. For the
    /// latter, the [`SRTO_RENDEZVOUS`](API-socket-options.md#
    /// SRTO_RENDEZVOUS) flag must be set to true prior to calling
    /// this function, and binding, as described in #1, is in this
    /// case obligatory (see `SRT_ERDVUNBOUND` below).
    ///
    /// 3. When [`u`](#u) is a group, then this call can be done multiple times,
    ///    each time
    /// for another member connection, and a new member SRT socket will be
    /// created automatically for every call of this function.
    ///
    /// 4. If you want to connect a group to multiple links at once and use
    ///    blocking
    /// mode, you might want to use
    /// [`srt_connect_group`](#srt_connect_group) instead. This
    /// function also allows you to use additional settings,
    /// available only for groups.
    ///
    /// If the `u` socket is configured for blocking mode (when
    /// [`SRTO_RCVSYN`](API-socket-options.md#SRTO_RCVSYN) is set to true,
    /// default), the call will block until the connection succeeds or
    /// fails. The "early" errors [`SRT_EINVSOCK`](#srt_einvsock),
    /// [`SRT_ERDVUNBOUND`](#srt_erdvunbound) and [`SRT_ECONNSOCK`](#
    /// srt_econnsock) are reported in both modes immediately. Other
    /// errors are "late" failures and can only be reported in blocking
    /// mode.
    ///
    /// In non-blocking mode, a successful connection can be recognized by
    /// the `SRT_EPOLL_OUT` epoll event flag and a "late" failure by
    /// the `SRT_EPOLL_ERR` flag. Note that the socket state in the
    /// case of a failed connection remains `SRTS_CONNECTING` in
    /// that case.
    ///
    /// In the case of "late" failures you can additionally call
    /// [`srt_getrejectreason`](#srt_getrejectreason) to get detailed error
    /// information. Note that in blocking mode only for the `SRT_ECONNREJ`
    /// error this function may return any additional information. In
    /// non-blocking mode a detailed "late" failure cannot be distinguished,
    /// and therefore it can also be obtained from this function.
    pub fn srt_connect(s: SRTSOCKET, name: *const sockaddr, name_len: c_int) -> c_int;
    /// Extracts the payload waiting to be received. Note that
    /// [`srt_recv`](#srt_recv) and [`srt_recvmsg`](#srt_recvmsg) are
    /// identical functions, two different names being kept for historical
    /// reasons. In the UDT predecessor the application was required
    /// to use either the `UDT::recv` version for **stream mode** and
    /// `UDT::recvmsg` for **message mode**. In SRT this distinction is
    /// resolved internally by the [`SRTO_MESSAGEAPI`](API-socket-options.
    /// md#SRTO_MESSAGEAPI) flag.
    ///
    /// **Arguments**:
    ///
    /// * [`u`](#u): Socket used to send. The socket must be connected for this
    ///   operation.
    /// * `buf`: Points to the buffer to which the payload is copied.
    /// * `len`: Size of the payload specified in `buf`.
    /// * `mctrl`: An object of [`SRT_MSGCTRL`](#SRT_MSGCTRL) type that contains
    ///   extra
    /// parameters.
    ///
    /// The way this function works is determined by the mode set in
    /// options, and it has specific requirements:
    ///
    /// 1. In **file/stream mode**, as many bytes as possible are retrieved,
    ///    that is,
    /// only so many bytes that fit in the buffer and are currently
    /// available. Any data that is available but not extracted this
    /// time will be available next time.
    ///
    /// 2. In **file/message mode**, exactly one message is retrieved, with the
    /// boundaries defined at the moment of sending. If some parts of the
    /// messages are already retrieved, but not the whole message, nothing
    /// will be received (the function blocks or returns
    /// [`SRT_EASYNCRCV`](#srt_easyncrcv)). If the message to be returned
    /// does not fit in the buffer, nothing will be received and
    /// the error is reported.
    ///
    /// 3. In **live mode**, the function behaves as in **file/message mode**,
    ///    although the
    /// number of bytes retrieved will be at most the maximum payload of one
    /// MTU. The [`SRTO_PAYLOADSIZE`](API-socket-options.md#
    /// SRTO_PAYLOADSIZE) value configured by the sender is not negotiated,
    /// and not known to the receiver.
    /// The [`SRTO_PAYLOADSIZE`](API-socket-options.md#SRTO_PAYLOADSIZE)
    /// value set on the SRT receiver is mainly used for heuristics.
    /// However, the receiver is prepared to receive the whole MTU
    /// as configured with [`SRTO_MSS`](API-socket-options.md#
    /// SRTO_MSS). In this mode, however, with default settings of
    /// [`SRTO_TSBPDMODE`](API-socket-options.md#SRTO_TSBPDMODE)
    /// and [`SRTO_TLPKTDROP`](API-socket-options.md#SRTO_TLPKTDROP), the
    /// message will be received only when its time to play has come, and
    /// until then it will be kept in the receiver buffer. Also, when the
    /// time to play has come for a message that is next to the currently
    /// lost one, it will be delivered and the lost one dropped.
    pub fn srt_recv(s: SRTSOCKET, buf: *mut c_char, len: c_int) -> c_int;
    /// Sends a payload to a remote party over a given socket.
    ///
    /// **Arguments**:
    ///
    /// * [`u`](#u): Socket used to send. The socket must be connected for this
    ///   operation.
    /// * `buf`: Points to the buffer containing the payload to send.
    /// * `len`: Size of the payload specified in `buf`.
    /// * `ttl`: Time (in `[ms]`) to wait for a successful delivery. See
    ///   description of
    /// the [`SRT_MSGCTRL::msgttl`](#SRT_MSGCTRL) field.
    /// * `inorder`: Required to be received in the order of sending. See
    /// [`SRT_MSGCTRL::inorder`](#SRT_MSGCTRL).
    /// * `mctrl`: An object of [`SRT_MSGCTRL`](#SRT_MSGCTRL) type that contains
    ///   extra
    /// parameters, including `ttl` and `inorder`.
    ///
    /// The way this function works is determined by the mode set in
    /// options, and it has specific requirements:
    ///
    /// 1. In **file/stream mode**, the payload is byte-based. You are not
    ///    required to
    /// know the size of the data, although they are only guaranteed to be
    /// received in the same byte order.
    ///
    /// 2. In **file/message mode**, the payload that you send using this
    ///    function is
    /// a single message that you intend to be received as a whole. In other
    /// words, a single call to this function determines a message's
    /// boundaries.
    ///
    /// 3. In **live mode**, you are only allowed to send up to the length of
    /// `SRTO_PAYLOADSIZE`, which can't be larger than 1456 bytes (1316
    /// default).
    ///
    /// **NOTE**: Note that in **file/stream mode** the returned size may be
    /// less than `len`, which means that it didn't send the whole contents
    /// of the buffer. You would need to call this function again with the
    /// rest of the buffer next time to send it completely. In both **file/
    /// message** and **live mode** the successful return is always equal to
    /// `len`.
    pub fn srt_send(s: SRTSOCKET, buf: *const c_char, len: c_int) -> c_int;
    /// Sets a value for a socket option in the socket or group.
    ///
    /// The first version (srt_setsockopt) follows the BSD socket API
    /// convention, although the "level" parameter is ignored. The second
    /// version (srt_setsockflag) omits the "level" parameter completely.
    ///
    /// Options correspond to various data types, so you need to know what
    /// data type is assigned to a particular option, and to pass a
    /// variable of the appropriate data type with the option value
    /// to be set.
    ///
    /// Please note that some of the options can only be set on sockets or
    /// only on groups, although most of the options can be set on
    /// the groups so that they are then derived by the member
    /// sockets.
    pub fn srt_setsockflag(
        s: SRTSOCKET,
        opt: SRT_SOCKOPT,
        optval: *const c_void,
        optlen: c_int,
    ) -> c_int;
    /// Extracts the address to which the socket was bound. Although you
    /// should know the address(es) that you have used for binding
    /// yourself, this function can be useful for extracting the
    /// local outgoing port number when it was specified as 0 with
    /// binding for system autoselection. With this function you can
    /// extract the port number after it has been autoselected.
    pub fn srt_getsockname(s: SRTSOCKET, addr: *mut sockaddr, addr_len: *mut c_int) -> c_int;
}
