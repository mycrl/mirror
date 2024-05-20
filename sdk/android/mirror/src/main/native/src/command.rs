use std::{cell::RefCell, sync::Mutex};

use bytes::{Bytes, BytesMut};
use jni::{objects::JByteArray, JNIEnv, JavaVM};

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
    pub static ENV: RefCell<Option<*mut jni::sys::JNIEnv>> = const { RefCell::new(None) };
}

pub static JVM: Mutex<Option<JavaVM>> = Mutex::new(None);

pub fn get_current_env<'local>() -> JNIEnv<'local> {
    unsafe {
        JNIEnv::from_raw(
            ENV.with(|cell| {
                let mut env = cell.borrow_mut();
                if env.is_none() {
                    let vm = JVM.lock().unwrap();
                    env.replace(
                        vm.as_ref()
                            .unwrap()
                            .attach_current_thread_as_daemon()
                            .unwrap()
                            .get_raw(),
                    );
                }

                *env
            })
            .unwrap(),
        )
        .unwrap()
    }
}

pub fn catcher<F, T>(env: &mut JNIEnv, func: F) -> Option<T>
where
    F: FnOnce(&mut JNIEnv) -> anyhow::Result<T>,
{
    match func(env) {
        Ok(ret) => Some(ret),
        Err(e) => {
            log::error!("java runtime exception, err={:?}", e);
            None
        }
    }
}

pub fn copy_from_byte_array(env: &JNIEnv, array: &JByteArray) -> anyhow::Result<Bytes> {
    let size = env.get_array_length(array)? as usize;
    let mut bytes = BytesMut::zeroed(size);
    env.get_byte_array_region(array, 0, unsafe {
        std::mem::transmute::<&mut [u8], &mut [i8]>(&mut bytes[..])
    })?;
    Ok(bytes.freeze())
}
