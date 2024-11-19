use std::{
    collections::HashMap,
    ffi::{c_char, c_void},
    ptr::null_mut,
};

use hylarana::DiscoveryService;
use hylarana_common::{
    c_str,
    strings::{write_c_str, Strings},
};

use super::log_error;

type Properties = HashMap<String, String>;

/// Create a properties.
#[no_mangle]
extern "C" fn hylarana_create_properties() -> *const Properties {
    Box::into_raw(Box::new(Properties::default()))
}

/// Adds key pair values to the property list, which is Map inside.
#[no_mangle]
extern "C" fn hylarana_properties_insert(
    properties: *mut Properties,
    key: *const c_char,
    value: *const c_char,
) -> bool {
    assert!(!properties.is_null());
    assert!(!value.is_null());
    assert!(!key.is_null());

    let func = || {
        unsafe { &mut *properties }.insert(
            Strings::from(key).to_string()?,
            Strings::from(value).to_string()?,
        );

        Ok::<_, anyhow::Error>(())
    };

    func().is_ok()
}

/// Get value from the property list, which is Map inside.
#[no_mangle]
extern "C" fn hylarana_properties_get(
    properties: *mut Properties,
    key: *const c_char,
    value: *mut c_char,
) -> bool {
    assert!(!properties.is_null());
    assert!(!value.is_null());
    assert!(!key.is_null());

    let key = if let Ok(it) = Strings::from(key).to_string() {
        it
    } else {
        return false;
    };

    if let Some(it) = unsafe { &mut *properties }.get(&key) {
        write_c_str(it, value);

        true
    } else {
        false
    }
}

/// Destroy the properties.
#[no_mangle]
extern "C" fn hylarana_properties_destroy(properties: *mut Properties) {
    assert!(!properties.is_null());

    drop(unsafe { Box::from_raw(properties) });
}

#[repr(C)]
struct RawDiscovery(DiscoveryService);

/// Register the service, the service type is fixed, you can customize the
/// port number, id is the identifying information of the service, used to
/// distinguish between different publishers, in properties you can add
/// customized data to the published service.
#[no_mangle]
extern "C" fn hylarana_discovery_register(
    port: u16,
    properties: *const Properties,
) -> *const RawDiscovery {
    let func =
        || Ok::<_, anyhow::Error>(DiscoveryService::register(port, unsafe { &*properties })?);

    log_error(func())
        .map(|it| Box::into_raw(Box::new(it)))
        .unwrap_or_else(|_| null_mut()) as *const _
}

type Callback = extern "C" fn(
    ctx: *const c_void,
    addrs: *const *const c_char,
    addrs_size: usize,
    properties: *const Properties,
);

struct CallbackWrap {
    callback: Callback,
    ctx: *const c_void,
}

unsafe impl Send for CallbackWrap {}
unsafe impl Sync for CallbackWrap {}

impl CallbackWrap {
    fn call(&self, addrs: Vec<String>, info: &Properties) {
        (self.callback)(
            self.ctx,
            addrs
                .iter()
                .map(|it| c_str!(it.as_str()))
                .collect::<Vec<_>>()
                .as_slice()
                .as_ptr(),
            addrs.len(),
            info,
        );
    }
}

/// Query the registered service, the service type is fixed, when the query
/// is published the callback function will call back all the network
/// addresses of the service publisher as well as the attribute information.
#[no_mangle]
extern "C" fn hylarana_discovery_query(
    callback: Callback,
    ctx: *const c_void,
) -> *const RawDiscovery {
    let callback = CallbackWrap { callback, ctx };
    let func = || {
        Ok::<_, anyhow::Error>(DiscoveryService::query(move |addrs, info| {
            callback.call(
                addrs.iter().map(|it| it.to_string()).collect::<Vec<_>>(),
                &info,
            );
        })?)
    };

    log_error(func())
        .map(|it| Box::into_raw(Box::new(it)))
        .unwrap_or_else(|_| null_mut()) as *const _
}

/// Destroy the discovery.
#[no_mangle]
extern "C" fn hylarana_discovery_destroy(discovery: *mut RawDiscovery) {
    assert!(!discovery.is_null());

    drop(unsafe { Box::from_raw(discovery) });
}
