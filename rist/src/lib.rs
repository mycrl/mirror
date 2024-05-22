mod receiver;
mod sender;

use std::{
    ffi::{c_char, c_int, c_void, CStr},
    ptr::null,
};

pub use receiver::{Packet, Receiver};
pub use sender::Sender;

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

extern "C" fn logging_proc(_: *const c_void, _: LogLevel, msg: *const c_char) -> c_int {
    if let Ok(log) = unsafe { CStr::from_ptr(msg) }.to_str() {
        print!("{}", log);
    }

    0
}

#[repr(C)]
#[allow(unused)]
pub enum Profile {
    Simple = 0,
    Main = 1,
    Advanced = 2,
}

#[repr(C)]
#[allow(unused)]
pub enum LogLevel {
    Disable = -1,
    Error = 3,
    Warn = 4,
    Notice = 5,
    Info = 6,
    Debug = 7,
    Simulate = 100,
}

#[repr(C)]
pub struct RawPacket {
    pub payload: *const u8,
    pub len: usize,
    pub ts_ntp: u64,
    pub virt_src_port: u16,
    pub virt_dst_port: u16,
    pub peer: *const c_void,
    pub flow_id: u32,
    pub seq: u64,
    pub flags: u32,
    pub rist_ref: *const c_void,
}

impl Default for RawPacket {
    fn default() -> Self {
        Self {
            rist_ref: null(),
            payload: null(),
            peer: null(),
            // The virtual source and destination ports are not used for simple profile
            virt_src_port: 0,
            // These next fields are not needed/used by rist_sender_data_write
            virt_dst_port: 0,
            flow_id: 0,
            ts_ntp: 0,
            flags: 0,
            // Get's populated by librist with the rtp_seq on output, can be used on input to tell
            // librist which rtp_seq to use
            seq: 0,
            len: 0,
        }
    }
}

extern "C" {
    /// populates and creates logging settings struct with log settings
    ///
    /// This also sets the global logging settings if they were not set before.
    ///
    /// @param logging_settings if pointed to pointer is NULL struct will be
    /// allocated, otherwise pointed to struct will have it's values updated by
    /// given values, closing and opening sockets as needed.
    /// @param log_level minimum log level to report
    /// @param log_cb log callback , NULL to disable
    /// @param cb_args user data passed to log callback function, NULL when
    /// unused @param address destination address for UDP log messages, NULL
    /// when unused @param logfp log file to write to, NULL when unused
    pub fn rist_logging_set(
        logging: *mut *const c_void,
        level: LogLevel,
        proc: extern "C" fn(*const c_void, LogLevel, *const c_char) -> c_int,
        data: *const c_void,
        address: *const c_char,
        fp: *const c_void,
    ) -> c_int;
    /// Add a peer to the RIST session
    ///
    /// One sender can send data to multiple peers.
    ///
    /// @param ctx RIST context
    /// @param[out] peer Store the new peer pointer
    /// @param config a pointer to the struct rist_peer_config, which contains
    ///        the configuration parameters for the peer endpoint.
    /// @return 0 on success, -1 in case of error.
    pub fn rist_peer_create(
        ctx: *const c_void,
        peer: *mut *const c_void,
        peer_config: *const c_void,
    ) -> c_int;
    /// Starts the RIST sender or receiver
    ///
    /// After all the peers have been added, this function triggers
    /// the RIST sender/receiver to start
    ///
    /// @param ctx RIST context
    /// @return 0 on success, -1 in case of error.
    pub fn rist_start(ctx: *const c_void) -> c_int;
    /// Destroy RIST sender/receiver
    ///
    /// Destroys the RIST instance
    ///
    /// @param ctx RIST context
    /// @return 0 on success, -1 on error
    pub fn rist_destroy(ctx: *const c_void) -> c_int;
    /// Receiver specific functions, use rist_receiver_create to create a
    /// receiver rist_ctx
    ///
    ///
    /// Create a RIST receiver instance
    ///
    /// @param[out] ctx a context representing the receiver instance
    /// @param profile RIST profile
    /// @param logging_settings Optional struct containing the logging
    /// settings. @return 0 on success, -1 on error
    pub fn rist_receiver_create(
        ctx: *mut *const c_void,
        profile: Profile,
        logging: *const c_void,
    ) -> c_int;
    /// Parses rist url for peer config data (encryption, compression, etc)
    ///
    /// Use this API to parse a generic URL string and turn it into a meaninful
    /// peer_config structure
    ///
    /// @param url a pointer to a url to be parsed, i.e.
    /// rist://myserver.net:1234?buffer=100&cname=hello @param[out]
    /// peer_config a pointer to a the rist_peer_config structure (NULL is
    /// allowed). When passing NULL, the library will allocate a new
    /// rist_peer_config structure with the latest default values and it
    /// expects the application to free it when it is done using it. @return
    /// 0 on success or non-zero on error. The value returned is actually the
    /// number of parameters that are valid
    pub fn rist_parse_address2(url: *const c_char, peer_config: *mut *const c_void) -> c_int;
    /// Enable data callback channel
    ///
    /// Call to enable data callback channel.
    ///
    /// @param ctx RIST receiver context
    ///  @param data_callback The function that will be called when a data frame
    /// is received from a sender.
    /// @param arg the extra argument passed to the `data_callback`
    /// @return 0 on success, -1 on error
    pub fn rist_receiver_data_callback_set2(
        ctx: *const c_void,
        proc: extern "C" fn(*const c_void, *const RawPacket) -> c_int,
        data: *const c_void,
    ) -> c_int;
    /// Create Sender
    ///
    /// Create a RIST sender instance
    ///
    /// @param[out] ctx a context representing the sender instance
    /// @param profile RIST profile
    /// @param flow_id Flow ID, use 0 to delegate creation of flow_id to lib
    /// @param logging_settings Struct containing logging settings
    /// @return 0 on success, -1 in case of error.
    pub fn rist_sender_create(
        ctx: *mut *const c_void,
        profile: Profile,
        flow_id: u32,
        logging: *const c_void,
    ) -> c_int;
    /// Write data into a librist packet.
    ///
    /// One sender can send write data into a librist packet.
    ///
    /// @param ctx RIST sender context
    /// @param data_block pointer to the rist_data_block structure
    /// the ts_ntp will be populated by the lib if a value of 0 is passed
    /// @return number of written bytes on success, -1 in case of error.
    pub fn rist_sender_data_write(ctx: *const c_void, data: *const RawPacket) -> c_int;
    /// Free rist data block
    ///
    /// Must be called whenever a received data block is no longer needed by the
    /// calling application.
    ///
    /// @param block double pointer to rist_data_block, containing pointer will
    /// be set to NULL
    pub fn rist_receiver_data_block_free2(data: *mut *const RawPacket);
}
