mod librist;

use std::{
    ffi::{c_char, c_int, c_void, CStr, CString},
    net::SocketAddr,
    ptr::null,
    sync::mpsc::{self, channel},
};

use librist::{
    rist_destroy, rist_logging_set, rist_parse_address2, rist_peer_create, rist_receiver_create,
    rist_receiver_data_block_free2, rist_receiver_data_callback_set2, rist_sender_create,
    rist_sender_data_write, rist_start, RistDataBlock, RistLevel, RistProfile,
};

struct Context(*const c_void);

impl Context {
    fn as_ptr(&self) -> *const c_void {
        self.0
    }
}

impl Drop for Context {
    fn drop(&mut self) {
        unsafe {
            rist_destroy(self.0);
        }
    }
}

#[derive(Debug)]
pub enum Error {
    SetLogging,
    CreateSender,
    ParseAddress,
    CreatePeer,
    Start,
    SendDataBlock,
    SetReceiverCallback,
}

impl std::error::Error for Error {}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                Self::SetLogging => "SetLogging",
                Self::CreateSender => "CreateSender",
                Self::ParseAddress => "ParseAddress",
                Self::CreatePeer => "CreatePeer",
                Self::Start => "Start",
                Self::SendDataBlock => "SendDataBlock",
                Self::SetReceiverCallback => "SetReceiverCallback",
            }
        )
    }
}

extern "C" fn logging_proc(_: *const c_void, _: RistLevel, msg: *const c_char) -> c_int {
    if let Ok(log) = unsafe { CStr::from_ptr(msg) }.to_str() {
        print!("{}", log);
    }

    0
}

pub struct Sender {
    context: Context,
    data: RistDataBlock,
}

unsafe impl Send for Sender {}
unsafe impl Sync for Sender {}

impl Sender {
    pub fn new(addr: SocketAddr) -> Result<Self, Error> {
        let mut logging_settings = null();
        if unsafe {
            rist_logging_set(
                &mut logging_settings,
                RistLevel::Debug,
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
        if unsafe { rist_sender_create(&mut ctx, RistProfile::Main, 0, logging_settings) } != 0 {
            return Err(Error::CreateSender);
        }

        let mut peer_config = null();
        let url = CString::new(format!("udp://{}", addr.to_string())).unwrap();
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

        Ok(Self {
            context: Context(ctx),
            data: Default::default(),
        })
    }

    pub fn send(&mut self, buf: &[u8]) -> Result<usize, Error> {
        self.data.payload = buf.as_ptr();
        self.data.len = buf.len();

        let ret = unsafe { rist_sender_data_write(self.context.as_ptr(), &self.data) };
        if ret < 0 {
            Err(Error::SendDataBlock)
        } else {
            Ok(ret as usize)
        }
    }
}

extern "C" fn receiver_proc(ctx: *const c_void, data: *const RistDataBlock) -> c_int {
    if unsafe { &*(ctx as *const mpsc::Sender<Packet>) }
        .send(Packet(data))
        .is_ok()
    {
        0
    } else {
        -1
    }
}

pub struct Packet(*const RistDataBlock);

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
                RistLevel::Debug,
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
        if unsafe { rist_receiver_create(&mut ctx, RistProfile::Main, logging_settings) } != 0 {
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
