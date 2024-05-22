use std::{fmt::Debug, mem::size_of};

use libc::{c_char, c_int};

use super::{srt_setsockflag, SrtError, SrtErrorKind, SRTSOCKET, SRT_SOCKOPT, SRT_TRANSTYPE};

#[derive(Debug, Clone)]
pub struct SrtOptions {
    pub max_bandwidth: i64,
    pub latency: u32,
    pub timeout: u32,
    pub fec: String,
    pub mtu: u32,
    pub fc: u32,
}

impl SrtOptions {
    pub(crate) fn apply_socket(&self, fd: i32) -> Result<(), SrtError> {
        set_sock_opt(fd, SRT_SOCKOPT::SRTO_TRANSTYPE, &SRT_TRANSTYPE::SRTT_LIVE)?;
        set_sock_opt(fd, SRT_SOCKOPT::SRTO_TSBPDMODE, &1_i32)?;
        set_sock_opt(fd, SRT_SOCKOPT::SRTO_TLPKTDROP, &1_i32)?;
        set_sock_opt(fd, SRT_SOCKOPT::SRTO_FC, &self.fc)?;
        set_sock_opt(fd, SRT_SOCKOPT::SRTO_MSS, &self.mtu)?;
        set_sock_opt(fd, SRT_SOCKOPT::SRTO_RCVLATENCY, &self.latency)?;
        set_sock_opt(fd, SRT_SOCKOPT::SRTO_MAXBW, &self.max_bandwidth)?;
        set_sock_opt(fd, SRT_SOCKOPT::SRTO_PEERIDLETIMEO, &self.timeout)?;
        set_sock_opt_str(fd, SRT_SOCKOPT::SRTO_PACKETFILTER, &self.fec)?;
        Ok(())
    }

    pub const fn max_pkt_size(&self) -> usize {
        (self.mtu as usize) - (1500 - 1316)
    }
}

impl Default for SrtOptions {
    fn default() -> Self {
        Self {
            fec: "fec,layout:even,rows:20,cols:10,arq:always".to_string(),
            max_bandwidth: -1,
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
) -> Result<(), SrtError> {
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
        SrtError::error(SrtErrorKind::SetOptError)
    }
}

fn set_sock_opt_str(sock: SRTSOCKET, opt: SRT_SOCKOPT, flag: &str) -> Result<(), SrtError> {
    if unsafe { srt_setsockflag(sock, opt, to_c_str(flag) as *const _, flag.len() as c_int) } == 0 {
        Ok(())
    } else {
        SrtError::error(SrtErrorKind::SetOptError)
    }
}

fn to_c_str(str: &str) -> *const c_char {
    std::ffi::CString::new(str).unwrap().into_raw()
}
