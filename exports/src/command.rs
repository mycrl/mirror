use std::cell::{OnceCell, RefCell};

use anyhow::anyhow;
use bytes::{Bytes, BytesMut};
use jni::{objects::JByteArray, JNIEnv};
use tokio::runtime::Runtime;

// The Tokio runtime.
//
// Unlike other Rust programs, asynchronous applications require runtime
// support. In particular, the following runtime services are necessary:
//
// An I/O event loop, called the driver, which drives I/O resources and
// dispatches I/O events to tasks that depend on them.
// A scheduler to execute tasks that use these I/O resources.
// A timer for scheduling work to run after a set period of time.
// Tokio’s Runtime bundles all of these services as a single type, allowing them
// to be started, shut down, and configured together. However, often it is not
// required to configure a Runtime manually, and a user may just use the
// tokio::main attribute macro, which creates a Runtime under the hood.
pub static mut RUNTIME: OnceCell<Runtime> = OnceCell::new();

// Each function is accessible at a fixed offset through the JNIEnv argument.
// The JNIEnv type is a pointer to a structure storing all JNI function
// pointers. It is defined as follows:
//
// typedef const struct JNINativeInterface *JNIEnv;
// The VM initializes the function table, as shown by the following code
// example. Note that the first three entries are reserved for future
// compatibility with COM. In addition, we reserve a number of additional NULL
// entries near the beginning of the function table, so that, for example, a
// future class-related JNI operation can be added after FindClass, rather than
// at the end of the table.
thread_local! {
    pub static ENV: RefCell<Option<*mut jni::sys::JNIEnv>> = RefCell::new(None);
}

pub fn get_current_env<'local>() -> JNIEnv<'local> {
    unsafe { JNIEnv::from_raw(ENV.with(|cell| *cell.borrow_mut()).unwrap()).unwrap() }
}

pub fn get_runtime() -> anyhow::Result<&'static Runtime> {
    unsafe { RUNTIME.get() }.ok_or_else(|| anyhow!("not found runtime."))
}

pub fn catcher<F, T>(env: &mut JNIEnv, func: F) -> Option<T>
where
    F: FnOnce(&mut JNIEnv) -> anyhow::Result<T>,
{
    match func(env) {
        Ok(ret) => Some(ret),
        Err(e) => {
            env.throw_new("java/lang/Exception", e.to_string()).unwrap();
            None
        }
    }
}

pub fn copy_from_byte_array(env: &JNIEnv, array: &JByteArray) -> anyhow::Result<Bytes> {
    let size = env.get_array_length(array)? as usize;
    let mut bytes = BytesMut::zeroed(size);
    env.get_byte_array_region(array, 0, unsafe { std::mem::transmute(&mut bytes[..]) })?;
    Ok(bytes.freeze())
}
