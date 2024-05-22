use crate::*;

use std::{
    ffi::{c_int, c_void, CString},
    net::SocketAddr,
    ptr::null,
    sync::mpsc::{self, channel},
};

pub struct Packet(*const RawPacket);

unsafe impl Send for Packet {}
unsafe impl Sync for Packet {}

impl Packet {
    pub fn as_slice(&self) -> &[u8] {
        let data = unsafe { &*self.0 };
        unsafe { std::slice::from_raw_parts(data.payload, data.len) }
    }
}

impl Drop for Packet {
    fn drop(&mut self) {
        unsafe { rist_receiver_data_block_free2(&mut self.0) }
    }
}

pub struct Receiver {
    tx: *mut mpsc::Sender<Packet>,
    rx: mpsc::Receiver<Packet>,
    #[allow(unused)]
    context: Context,
}

unsafe impl Send for Receiver {}
unsafe impl Sync for Receiver {}

impl Receiver {
    pub fn new(addr: SocketAddr) -> Result<Self, Error> {
        let mut logging_settings = null();
        if unsafe {
            rist_logging_set(
                &mut logging_settings,
                LogLevel::Debug,
                logging_proc,
                null(),
                null(),
                null(),
            )
        } != 0
        {
            return Err(Error::SetLogging);
        }

        let mut ctx = null();
        if unsafe { rist_receiver_create(&mut ctx, Profile::Main, logging_settings) } != 0 {
            return Err(Error::CreateSender);
        }

        let mut peer_config = null();
        let url = CString::new(format!("udp://@{}", addr.to_string())).unwrap();
        if unsafe { rist_parse_address2(url.as_ptr(), &mut peer_config) } != 0 {
            return Err(Error::ParseAddress);
        }

        let mut peer = null();
        if unsafe { rist_peer_create(ctx, &mut peer, peer_config) } != 0 {
            return Err(Error::CreatePeer);
        }

        if unsafe { rist_start(ctx) } != 0 {
            return Err(Error::Start);
        }

        let (tx, rx) = channel();
        let tx = Box::into_raw(Box::new(tx));
        if unsafe { rist_receiver_data_callback_set2(ctx, receiver_proc, tx as *const _) } != 0 {
            return Err(Error::SetReceiverCallback);
        }

        Ok(Self {
            context: Context(ctx),
            tx,
            rx,
        })
    }

    pub fn read(&self) -> Option<Packet> {
        self.rx.recv().ok()
    }
}

impl Drop for Receiver {
    fn drop(&mut self) {
        drop(unsafe { Box::from_raw(self.tx) })
    }
}

extern "C" fn receiver_proc(ctx: *const c_void, data: *const RawPacket) -> c_int {
    if unsafe { &*(ctx as *const mpsc::Sender<Packet>) }
        .send(Packet(data))
        .is_ok()
    {
        0
    } else {
        -1
    }
}
