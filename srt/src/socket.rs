use std::{
    net::SocketAddr,
    sync::{atomic::AtomicBool, Arc},
};

use libc::c_int;
use os_socketaddr::OsSocketAddr;
use sync::atomic::EasyAtomic;
use tokio::runtime::Handle;

use crate::{
    api::{
        srt_bstats, srt_close, srt_connect, srt_create_socket, srt_recv, srt_send, SRTSOCKET,
        SRT_INVALID_SOCK,
    },
    options::SrtOptions,
    SrtError, SrtErrorKind, TraceStats,
};

pub struct Socket {
    fd: SRTSOCKET,
    opt: SrtOptions,
    is_closed: Arc<AtomicBool>,
}

impl Socket {
    pub(crate) fn new(fd: SRTSOCKET, opt: SrtOptions) -> Self {
        Self {
            is_closed: Arc::new(AtomicBool::new(false)),
            opt,
            fd,
        }
    }

    /// Connects a socket or a group to a remote party with a specified address
    /// and port.
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
    /// predefined network interface or local outgoing port. This is optional
    /// in the case of a caller-listener arrangement, but obligatory for a
    /// rendezvous arrangement. If not used, the binding will be done
    /// automatically to `INADDR_ANY` (which binds on all interfaces) and
    /// port 0 (which makes the system assign the port automatically).
    ///
    /// 2. This function is used for both connecting to the listening peer in a
    ///    caller-listener
    /// arrangement, and calling the peer in rendezvous mode. For the latter,
    /// the [`SRTO_RENDEZVOUS`](API-socket-options.md#SRTO_RENDEZVOUS) flag
    /// must be set to true prior to calling this function, and binding, as
    /// described in #1, is in this case obligatory (see `SRT_ERDVUNBOUND`
    /// below).
    ///
    /// 3. When [`u`](#u) is a group, then this call can be done multiple times,
    ///    each time
    /// for another member connection, and a new member SRT socket will be
    /// created automatically for every call of this function.
    ///
    /// 4. If you want to connect a group to multiple links at once and use
    ///    blocking
    /// mode, you might want to use [`srt_connect_group`](#srt_connect_group)
    /// instead. This function also allows you to use additional settings,
    /// available only for groups.
    ///
    /// If the `u` socket is configured for blocking mode (when
    /// [`SRTO_RCVSYN`](API-socket-options.md#SRTO_RCVSYN) is set to true,
    /// default), the call will block until the connection succeeds or
    /// fails. The "early" errors [`SRT_EINVSOCK`](#srt_einvsock),
    /// [`SRT_ERDVUNBOUND`](#srt_erdvunbound) and [`SRT_ECONNSOCK`](#
    /// srt_econnsock) are reported in both modes immediately. Other
    /// errors are "late" failures and can only be reported in blocking mode.
    ///
    /// In non-blocking mode, a successful connection can be recognized by the
    /// `SRT_EPOLL_OUT` epoll event flag and a "late" failure by the
    /// `SRT_EPOLL_ERR` flag. Note that the socket state in the case of a
    /// failed connection remains `SRTS_CONNECTING` in that case.
    ///
    /// In the case of "late" failures you can additionally call
    /// [`srt_getrejectreason`](#srt_getrejectreason) to get detailed error
    /// information. Note that in blocking mode only for the `SRT_ECONNREJ`
    /// error this function may return any additional information. In
    /// non-blocking mode a detailed "late" failure cannot be distinguished,
    /// and therefore it can also be obtained from this function.
    pub async fn connect(addr: SocketAddr, opt: SrtOptions) -> Result<Self, SrtError> {
        Handle::current()
            .spawn_blocking(move || {
                let fd = unsafe { srt_create_socket() };
                if fd == SRT_INVALID_SOCK {
                    return SrtError::error(SrtErrorKind::InvalidSock);
                } else {
                    opt.apply_socket(fd)?;
                }

                let addr: OsSocketAddr = addr.into();
                if unsafe { srt_connect(fd, addr.as_ptr() as *const _, addr.len() as c_int) } == -1
                {
                    return SrtError::error(SrtErrorKind::ConnectError);
                }

                Ok(Self::new(fd, opt))
            })
            .await
            .expect("not run tokio spawn blocking")
    }

    /// Reports the current statistics
    ///
    /// Arguments:
    ///
    /// u: Socket from which to get statistics
    /// perf: Pointer to an object to be written with the statistics
    /// clear: 1 if the statistics should be cleared after retrieval
    pub fn get_stats(&self) -> Result<TraceStats, SrtError> {
        let mut stats = TraceStats::default();
        if unsafe { srt_bstats(self.fd, &mut stats, true as i32) } != 0 {
            return SrtError::error(SrtErrorKind::GetStatsError);
        }

        Ok(stats)
    }

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
    /// The way this function works is determined by the mode set in options,
    /// and it has specific requirements:
    ///
    /// 1. In **file/stream mode**, as many bytes as possible are retrieved,
    ///    that is,
    /// only so many bytes that fit in the buffer and are currently available.
    /// Any data that is available but not extracted this time will be
    /// available next time.
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
    /// The [`SRTO_PAYLOADSIZE`](API-socket-options.md#SRTO_PAYLOADSIZE) value
    /// set on the SRT receiver is mainly used for heuristics. However, the
    /// receiver is prepared to receive the whole MTU as configured with
    /// [`SRTO_MSS`](API-socket-options.md#SRTO_MSS). In this mode, however,
    /// with default settings of
    /// [`SRTO_TSBPDMODE`](API-socket-options.md#SRTO_TSBPDMODE)
    /// and [`SRTO_TLPKTDROP`](API-socket-options.md#SRTO_TLPKTDROP), the
    /// message will be received only when its time to play has come, and
    /// until then it will be kept in the receiver buffer. Also, when the
    /// time to play has come for a message that is next to the currently
    /// lost one, it will be delivered and the lost one dropped.
    pub fn read(&self, buf: &mut [u8]) -> Result<usize, SrtError> {
        if self.is_closed.get() {
            return SrtError::error(SrtErrorKind::RecvError);
        }

        let ret = unsafe { srt_recv(self.fd, buf.as_mut_ptr() as *mut _, buf.len() as c_int) };
        if ret <= 0 {
            self.is_closed.update(true);
        }

        if ret < 0 {
            SrtError::error(SrtErrorKind::RecvError)
        } else {
            Ok(ret as usize)
        }
    }

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
    /// The way this function works is determined by the mode set in options,
    /// and it has specific requirements:
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
    pub fn send(&self, mut buf: &[u8]) -> Result<(), SrtError> {
        if buf.len() == 0 {
            return Ok(());
        }

        while !buf.is_empty() {
            buf = &buf[self.send_with_sized(buf)?..];
        }

        Ok(())
    }

    fn send_with_sized(&self, buf: &[u8]) -> Result<usize, SrtError> {
        if buf.len() == 0 {
            return Ok(0);
        }

        if self.is_closed.get() {
            return SrtError::error(SrtErrorKind::SendError);
        }

        let size = std::cmp::min(buf.len(), self.opt.max_pkt_size());
        let ret = unsafe { srt_send(self.fd, buf.as_ptr() as *const _, size as c_int) } as usize;
        if ret != size {
            self.is_closed.update(true);
            SrtError::error(SrtErrorKind::SendError)
        } else {
            Ok(ret as usize)
        }
    }
}

impl Drop for Socket {
    fn drop(&mut self) {
        if unsafe { srt_close(self.fd) } != 0 {
            log::error!("not release the socket!");
        }
    }
}
