use std::{
    ffi::{c_char, c_int},
    mem::size_of,
    net::SocketAddr,
    os::raw::c_void,
};

use libc::sockaddr;
use os_socketaddr::OsSocketAddr;
use tokio::{
    runtime::Handle,
    sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender},
};

use crate::{
    api::{
        srt_bind, srt_bstats, srt_close, srt_create_socket, srt_getsockname, srt_listen,
        srt_listen_callback, SRTSOCKET, SRT_INVALID_SOCK,
    },
    options::SrtOptions,
    socket::Socket,
    SrtError, SrtErrorKind, TraceStats,
};

pub struct Listener {
    ctx: *mut UnboundedSender<(SRTSOCKET, SocketAddr)>,
    rx: UnboundedReceiver<(SRTSOCKET, SocketAddr)>,
    fd: SRTSOCKET,
    opt: SrtOptions,
}

unsafe impl Send for Listener {}
unsafe impl Sync for Listener {}

impl Listener {
    /// Binds a socket to a local address and port. Binding specifies the local
    /// network interface and the UDP port number to be used for the socket.
    /// When the local address is a wildcard (`INADDR_ANY` for IPv4 or
    /// `in6addr_any` for IPv6), then it's bound to all interfaces.
    ///
    /// **IMPORTANT**: When you bind an IPv6 wildcard address, note that the
    /// `SRTO_IPV6ONLY` option must be set on the socket explicitly to 1 or 0
    /// prior to calling this function. See
    /// [`SRTO_IPV6ONLY`](API-socket-options.md#SRTO_IPV6ONLY) for more details.
    ///
    /// Binding is necessary for every socket to be used for communication. If
    /// the socket is to be used to initiate a connection to a listener
    /// socket, which can be done, for example, by the
    /// [`srt_connect`](#srt_connect) function, the socket is bound
    /// implicitly to the wildcard address according to the IP family
    /// (`INADDR_ANY` for `AF_INET` or `in6addr_any` for `AF_INET6`) and
    /// port number 0. In all other cases, a socket must be bound explicitly
    /// by using the functionality of this function first.
    ///
    /// When the port number parameter is 0, then the effective port number will
    /// be system-allocated. To obtain this effective port number you can
    /// use [`srt_getsockname`](#srt_getsockname).
    ///
    /// This call is obligatory for a listening socket before calling
    /// [`srt_listen`](#srt_listen) and for rendezvous mode before calling
    /// [`srt_connect`](#srt_connect); otherwise it's optional. For a
    /// listening socket it defines the network interface and the port where
    /// the listener should expect a call request.
    ///
    /// In the case of rendezvous mode there are two parties that connect to one
    /// another. For every party there must be chosen a local binding
    /// endpoint (local address and port) to which they expect connection
    /// from the peer. Let's say, we have a Party 1 that selects an endpoint
    /// A and a Party 2 that selects an endpoint B. In this case the Party 1
    /// binds the socket to the endpoint A and then connects to the endpoint B,
    /// and the Party 2 the other way around. Both sockets must be set
    /// [`SRTO_RENDEZVOUS`](API-socket-options.md#SRTO_RENDEZVOUS) to *true* to
    /// make this connection possible.
    ///
    /// For a connecting socket the call to `srt_bind` is optional, but can be
    /// used to set up the outgoing port for communication as well as the
    /// local interface through which it should reach out to the remote
    /// endpoint, should that be necessary.
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
    /// it will report `SRT_ESOCKFAIL`. Here is the table that shows possible
    /// situations:
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
    /// sharing the underlying UDP socket and communication queues between SRT
    /// sockets. If all existing bindings on the same port are "free" then
    /// the requested binding will allocate a distinct UDP socket for this
    /// SRT socket ("side binding").
    ///
    /// **NOTE**: This function cannot be called on a socket group. If you need
    /// to have the group-member socket bound to the specified source
    /// address before connecting, use
    /// [`srt_connect_bind`](#srt_connect_bind) for that purpose or set the
    /// appropriate source address using [`srt_prepare_endpoint`](#
    /// srt_prepare_endpoint).
    ///
    /// **IMPORTANT information about IPv6**: If you are going to bind to the
    /// `in6addr_any` IPv6 wildcard address (known as `::`), the `SRTO_IPV6ONLY`
    /// option must be first set explicitly to 0 or 1, otherwise the binding
    /// will fail. In all other cases this option is meaningless. See
    /// `SRTO_IPV6ONLY` option for more information.
    pub async fn bind(addr: SocketAddr, opt: SrtOptions, backlog: u32) -> Result<Self, SrtError> {
        Handle::current()
            .spawn_blocking(move || {
                let fd = unsafe { srt_create_socket() };
                if fd == SRT_INVALID_SOCK {
                    return SrtError::error(SrtErrorKind::InvalidSock);
                } else {
                    opt.apply_socket(fd)?;
                }

                let addr: OsSocketAddr = addr.into();
                if unsafe { srt_bind(fd, addr.as_ptr() as *const _, addr.len() as c_int) } == -1 {
                    return SrtError::error(SrtErrorKind::BindError);
                }

                if unsafe { srt_listen(fd, backlog as c_int) } == -1 {
                    return SrtError::error(SrtErrorKind::ListenError);
                }

                let (tx, rx) = unbounded_channel();
                let ctx = Box::into_raw(Box::new(tx));
                if unsafe { srt_listen_callback(fd, listener_fn, ctx as *mut _) } != 0 {
                    SrtError::error(SrtErrorKind::ListenError)
                } else {
                    Ok(Self { fd, ctx, rx, opt })
                }
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

    /// Accepts a pending connection, then creates and returns a new socket or
    /// group ID that handles this connection. The group and socket can be
    /// distinguished by checking the `SRTGROUP_MASK` bit on the returned ID.
    ///
    /// * `lsn`: the listener socket previously configured by
    ///   [`srt_listen`](#srt_listen)
    /// * `addr`: the IP address and port specification for the remote party
    /// * `addrlen`: INPUT: size of `addr` pointed object. OUTPUT: real size of
    ///   the
    /// returned object
    ///
    /// **NOTE:** `addr` is allowed to be NULL, in which case it's understood
    /// that the application is not interested in the address from which the
    /// connection originated. Otherwise `addr` should specify an object
    /// into which the address will be written, and `addrlen` must also
    /// specify a variable to contain the object size. Note also that in the
    /// case of group connection only the initial connection that
    /// establishes the group connection is returned, together with its address.
    /// As member connections are added or broken within the group, you can
    /// obtain this information through [`srt_group_data`](#srt_group_data)
    /// or the data filled by [`srt_sendmsg2`](#srt_sendmsg) and
    /// [`srt_recvmsg2`](#srt_recvmsg2).
    ///
    /// If the `lsn` listener socket is configured for blocking mode
    /// ([`SRTO_RCVSYN`](API-socket-options.md#SRTO_RCVSYN) set to true,
    /// default), the call will block until the incoming connection is
    /// ready. Otherwise, the call always returns immediately. The
    /// `SRT_EPOLL_IN` epoll event should be checked on the `lsn` socket
    /// prior to calling this function in that case.
    ///
    /// If the pending connection is a group connection (initiated on the peer
    /// side by calling the connection function using a group ID, and
    /// permitted on the listener socket by the
    /// [`SRTO_GROUPCONNECT`](API-socket-options.md#SRTO_GROUPCONNECT)
    /// flag), then the value returned is a group ID. This function then creates
    /// a new group, as well as a new socket for this connection, that will
    /// be added to the group. Once the group is created this way, further
    /// connections within the same group, as well as sockets for them, will
    /// be created in the background. The [`SRT_EPOLL_UPDATE`](#
    /// SRT_EPOLL_UPDATE) event is raised on the `lsn` socket when
    /// a new background connection is attached to the group, although it's
    /// usually for internal use only.
    pub async fn accept(&mut self) -> Result<(Socket, SocketAddr), SrtError> {
        if let Some((fd, addr)) = self.rx.recv().await {
            Ok((Socket::new(fd, self.opt.clone()), addr))
        } else {
            SrtError::error(SrtErrorKind::AcceptError)
        }
    }

    /// Extracts the address to which the socket was bound. Although you should
    /// know the address(es) that you have used for binding yourself, this
    /// function can be useful for extracting the local outgoing port number
    /// when it was specified as 0 with binding for system autoselection. With
    /// this function you can extract the port number after it has been
    /// autoselected.
    pub fn local_addr(&self) -> Option<SocketAddr> {
        let mut addr = OsSocketAddr::new();
        let mut addrlen = addr.capacity() as c_int;
        unsafe {
            srt_getsockname(self.fd, addr.as_mut_ptr() as *mut _, &mut addrlen);
        }

        addr.into()
    }
}

impl Drop for Listener {
    fn drop(&mut self) {
        unsafe {
            let _ = Box::from_raw(self.ctx);
            if srt_close(self.fd) != 0 {
                log::error!("not release the listener!");
            }
        }
    }
}

extern "C" fn listener_fn(
    opaque: *mut c_void,
    s: SRTSOCKET,
    _hs_version: c_int,
    peeraddr: *const sockaddr,
    _streamid: *const c_char,
) -> c_int {
    #[cfg(not(target_os = "windows"))]
    use libc::sockaddr_in;

    #[cfg(target_os = "windows")]
    let size = size_of::<sockaddr>() as i32;

    #[cfg(not(target_os = "windows"))]
    let size = size_of::<sockaddr_in>() as u32;

    let tx = unsafe { &*(opaque as *mut UnboundedSender<(SRTSOCKET, SocketAddr)>) };
    if let Some(addr) = unsafe { OsSocketAddr::copy_from_raw(peeraddr as *const _, size) }.into() {
        if tx.send((s, addr)).is_ok() {
            0
        } else {
            -1
        }
    } else {
        -1
    }
}
