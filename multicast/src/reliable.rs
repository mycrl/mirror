use std::ffi::{c_char, c_double, c_float, c_int, c_void};

#[repr(C)]
struct RawReliableConfig {
    name: [c_char; 256],
    context: *mut c_void,
    id: u64,
    max_packet_size: c_int,
    fragment_above: c_int,
    max_fragments: c_int,
    fragment_size: c_int,
    ack_buffer_size: c_int,
    sent_packets_buffer_size: c_int,
    received_packets_buffer_size: c_int,
    fragment_reassembly_buffer_size: c_int,
    rtt_smoothing_factor: c_float,
    packet_loss_smoothing_factor: c_float,
    bandwidth_smoothing_factor: c_float,
    packet_header_size: c_int,
    transmit_packet_function: extern "C" fn(*mut c_void, u64, u16, *const u8, c_int),
    process_packet_function: extern "C" fn(*mut c_void, u64, u16, *const u8, c_int) -> c_int,
    allocator_context: *const c_void,
    allocate_function: extern "C" fn(*const c_void, usize),
    free_function: extern "C" fn(*const c_void, *const c_void),
}

impl Default for RawReliableConfig {
    fn default() -> Self {
        let mut this = std::mem::MaybeUninit::<Self>::uninit();
        unsafe { reliable_default_config(this.as_mut_ptr()) }
        unsafe { this.assume_init() }
    }
}

extern "C" {
    fn reliable_default_config(config: *mut RawReliableConfig);
    fn reliable_endpoint_create(config: *const RawReliableConfig, time: c_double) -> *const c_void;
    fn reliable_endpoint_receive_packet(endpoint: *const c_void, data: *const u8, size: c_int);
    fn reliable_endpoint_send_packet(endpoint: *const c_void, data: *const u8, size: c_int);
    fn reliable_endpoint_get_acks(endpoint: *const c_void, acks: *mut c_int) -> *const u16;
    fn reliable_endpoint_clear_acks(endpoint: *const c_void);
    fn reliable_endpoint_update(endpoint: *const c_void, time: c_double);
    fn reliable_endpoint_destroy(endpoint: *const c_void);
    fn reliable_copy_string(dest: *mut c_char, source: *const c_char, size: usize);
}

#[derive(Debug, Clone)]
pub struct ReliableConfig {
    pub name: String,
    pub max_packet_size: usize,
    pub max_fragment_size: usize,
    pub max_fragments: usize,
    pub fragment_size: usize,
}

pub trait ReliableObserver {
    fn send(&mut self, id: u64, sequence: u16, buf: &[u8]);

    #[allow(unused_variables)]
    fn recv(&mut self, id: u64, sequence: u16, buf: &[u8]) -> bool {
        false
    }
}

pub struct Reliable {
    endpoint: *const c_void,
    config: RawReliableConfig,
}

unsafe impl Send for Reliable {}
unsafe impl Sync for Reliable {}

impl Reliable {
    pub fn new<T: ReliableObserver + 'static>(
        options: ReliableConfig,
        time: f64,
        observer: T,
    ) -> Self {
        let mut config = RawReliableConfig::default();
        config.context = Box::into_raw(Box::new(Context(Box::new(observer)))) as *mut _;
        config.transmit_packet_function = transmit_packet_function;
        config.process_packet_function = process_packet_function;
        config.max_packet_size = options.max_packet_size as c_int;
        config.fragment_above = options.max_fragment_size as c_int;
        config.max_fragments = options.max_fragments as c_int;
        config.fragment_size = options.fragment_size as c_int;

        unsafe {
            reliable_copy_string(
                config.name.as_mut_ptr(),
                options.name.as_ptr() as *const _,
                256,
            )
        }

        let endpoint = unsafe { reliable_endpoint_create(&config, time) };
        if endpoint.is_null() {
            panic!("Unable to create reliable transport module, this is a fatal error")
        } else {
            Self { endpoint, config }
        }
    }

    pub fn update(&self, time: f64) {
        unsafe { reliable_endpoint_update(self.endpoint, time) }

        let mut len = 0;
        let acks = unsafe { reliable_endpoint_get_acks(self.endpoint, &mut len) };
        let acks = unsafe { std::slice::from_raw_parts(acks, len as usize) };
        if !acks.is_empty() {
            unsafe { reliable_endpoint_clear_acks(self.endpoint) }
        }
    }

    pub fn send(&mut self, buf: &[u8]) {
        unsafe { reliable_endpoint_send_packet(self.endpoint, buf.as_ptr(), buf.len() as c_int) }
    }

    pub fn recv(&mut self, buf: &[u8]) {
        unsafe { reliable_endpoint_receive_packet(self.endpoint, buf.as_ptr(), buf.len() as c_int) }
    }
}

impl Drop for Reliable {
    fn drop(&mut self) {
        drop(unsafe { Box::from_raw(self.config.context as *mut Context) });
        unsafe { reliable_endpoint_destroy(self.endpoint) }
    }
}

struct Context(Box<dyn ReliableObserver>);

extern "C" fn transmit_packet_function(
    ctx: *mut c_void,
    id: u64,
    sequence: u16,
    buf: *const u8,
    size: c_int,
) {
    unsafe { &mut *(ctx as *mut Context) }
        .0
        .send(id, sequence, unsafe {
            std::slice::from_raw_parts(buf, size as usize)
        })
}

extern "C" fn process_packet_function(
    ctx: *mut c_void,
    id: u64,
    sequence: u16,
    buf: *const u8,
    size: c_int,
) -> c_int {
    if unsafe { &mut *(ctx as *mut Context) }
        .0
        .recv(id, sequence, unsafe {
            std::slice::from_raw_parts(buf, size as usize)
        })
    {
        1
    } else {
        0
    }
}
