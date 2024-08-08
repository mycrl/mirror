use std::{
    ffi::{c_char, c_int},
    fmt::Debug,
    io::Error,
    mem::size_of,
};

use utils::strings::Strings;

use super::{error, srt_getsockflag, srt_setsockflag, SRTSOCKET, SRT_SOCKOPT, SRT_TRANSTYPE};

#[derive(Debug, Clone)]
pub struct Options {
    pub stream_id: Option<String>,
    pub max_bandwidth: i64,
    pub latency: u32,
    pub timeout: u32,
    pub fec: String,
    pub mtu: u32,
    pub fc: u32,
}

impl Options {
    pub(crate) fn apply_socket(&self, fd: i32) -> Result<(), Error> {
        set_sock_opt(fd, SRT_SOCKOPT::SRTO_TRANSTYPE, &SRT_TRANSTYPE::SRTT_LIVE)?;
        set_sock_opt(fd, SRT_SOCKOPT::SRTO_RCVSYN, &1_i32)?;
        set_sock_opt(fd, SRT_SOCKOPT::SRTO_SNDSYN, &0_i32)?;
        set_sock_opt(fd, SRT_SOCKOPT::SRTO_TSBPDMODE, &1_i32)?;
        set_sock_opt(fd, SRT_SOCKOPT::SRTO_TLPKTDROP, &1_i32)?;
        set_sock_opt(fd, SRT_SOCKOPT::SRTO_FC, &self.fc)?;
        set_sock_opt(fd, SRT_SOCKOPT::SRTO_MSS, &self.mtu)?;
        set_sock_opt(fd, SRT_SOCKOPT::SRTO_RCVLATENCY, &self.latency)?;
        set_sock_opt(fd, SRT_SOCKOPT::SRTO_MAXBW, &self.max_bandwidth)?;
        set_sock_opt(fd, SRT_SOCKOPT::SRTO_PEERIDLETIMEO, &self.timeout)?;
        set_sock_opt_str(fd, SRT_SOCKOPT::SRTO_PACKETFILTER, &self.fec)?;

        if let Some(stream_id) = &self.stream_id {
            set_sock_opt_str(fd, SRT_SOCKOPT::SRTO_STREAMID, stream_id)?;
        }

        Ok(())
    }

    pub const fn max_pkt_size(&self) -> usize {
        (self.mtu as usize) - (1500 - 1316)
    }
}

impl Default for Options {
    fn default() -> Self {
        Self {
            fec: "fec,layout:staircase,rows:5,cols:10,arq:onreq".to_string(),
            max_bandwidth: -1,
            stream_id: None,
            timeout: 5000,
            latency: 120,
            mtu: 1500,
            fc: 25600,
        }
    }
}

fn set_sock_opt<T: Sized + Debug + PartialEq>(
    sock: SRTSOCKET,
    opt: SRT_SOCKOPT,
    flag: &T,
) -> Result<(), Error> {
    if unsafe {
        srt_setsockflag(
            sock,
            opt,
            flag as *const T as *const _,
            size_of::<T>() as c_int,
        )
    } == 0
    {
        Ok(())
    } else {
        Err(error())
    }
}

fn set_sock_opt_str(sock: SRTSOCKET, opt: SRT_SOCKOPT, flag: &str) -> Result<(), Error> {
    if unsafe {
        srt_setsockflag(
            sock,
            opt,
            Strings::from(flag).as_ptr() as *const _,
            flag.len() as c_int,
        )
    } == 0
    {
        Ok(())
    } else {
        Err(error())
    }
}

pub(crate) fn get_sock_opt_str(sock: SRTSOCKET, opt: SRT_SOCKOPT) -> Option<String> {
    let mut optval: [c_char; 521] = [0; 521];
    let mut optlen = 521;

    if unsafe { srt_getsockflag(sock, opt, optval.as_mut_ptr() as *mut _, &mut optlen) } == 0 {
        Strings::from(optval.as_ptr()).to_string().ok()
    } else {
        None
    }
}
