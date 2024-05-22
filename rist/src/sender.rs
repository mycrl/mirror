use crate::*;

use std::{ffi::CString, net::SocketAddr, ptr::null};

pub struct Sender {
    context: Context,
    data: RawPacket,
}

unsafe impl Send for Sender {}
unsafe impl Sync for Sender {}

impl Sender {
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
        if unsafe { rist_sender_create(&mut ctx, Profile::Main, 0, logging_settings) } != 0 {
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
