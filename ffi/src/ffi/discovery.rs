use std::{
    collections::HashMap,
    ffi::{c_char, c_void},
    ptr::null_mut,
};

use hylarana::DiscoveryService;
use hylarana_common::{c_str, strings::Strings};
use serde::{Deserialize, Serialize};

use super::log_error;

#[repr(C)]
#[derive(Default, Debug, Serialize, Deserialize)]
struct RawProperties(HashMap<String, String>);

#[no_mangle]
extern "C" fn hylarana_create_properties() -> *const RawProperties {
    Box::into_raw(Box::new(RawProperties::default()))
}

#[no_mangle]
extern "C" fn hylarana_properties_insert(
    properties: *mut RawProperties,
    key: *const c_char,
    value: *const c_char,
) -> bool {
    assert!(!properties.is_null());
    assert!(!value.is_null());
    assert!(!key.is_null());

    let func = || {
        unsafe { &mut *properties }.0.insert(
            Strings::from(key).to_string()?,
            Strings::from(value).to_string()?,
        );

        Ok::<_, anyhow::Error>(())
    };

    func().is_ok()
}

#[no_mangle]
extern "C" fn hylarana_properties_destroy(properties: *mut RawProperties) {
    assert!(!properties.is_null());

    drop(unsafe { Box::from_raw(properties) });
}

#[repr(C)]
struct RawDiscovery(DiscoveryService);

#[no_mangle]
extern "C" fn hylarana_discovery_register(
    port: u16,
    id: *const c_char,
    properties: *const RawProperties,
) -> *const RawDiscovery {
    let func = || {
        Ok::<_, anyhow::Error>(DiscoveryService::register(
            port,
            &Strings::from(id).to_string()?,
            unsafe { &*properties },
        )?)
    };

    log_error(func())
        .map(|it| Box::into_raw(Box::new(it)))
        .unwrap_or_else(|_| null_mut()) as *const _
}

type Callback = extern "C" fn(
    ctx: *const c_void,
    addrs: *const *const c_char,
    properties: *const RawProperties,
) -> bool;

struct CallbackWrap {
    callback: Callback,
    ctx: *const c_void,
}

unsafe impl Send for CallbackWrap {}
unsafe impl Sync for CallbackWrap {}

impl CallbackWrap {
    fn call(&self, addrs: Vec<String>, info: &RawProperties) {
        (self.callback)(
            self.ctx,
            addrs
                .iter()
                .map(|it| c_str!(it.as_str()))
                .collect::<Vec<_>>()
                .as_slice()
                .as_ptr(),
            info,
        );
    }
}

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

#[no_mangle]
extern "C" fn hylarana_discovery_destroy(discovery: *mut RawDiscovery) {
    assert!(!discovery.is_null());

    drop(unsafe { Box::from_raw(discovery) });
}
