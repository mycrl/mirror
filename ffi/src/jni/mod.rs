mod discovery;
mod object;
mod receiver;
mod sender;

use std::{
    cell::RefCell,
    collections::HashMap,
    ffi::c_void,
    ptr::null_mut,
    sync::{Arc, Mutex},
    thread,
};

use anyhow::Result;
use discovery::DiscoveryServiceObserver;
use hylarana_common::logger;
use hylarana_discovery::DiscoveryService;
use jni::{
    objects::{JByteArray, JClass, JObject, JString},
    sys::{jint, JNI_VERSION_1_6},
    JNIEnv, JavaVM,
};

use self::{
    object::{TransformArray, TransformMap},
    receiver::Receiver,
    sender::Sender,
};

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

static JVM: Mutex<Option<JavaVM>> = Mutex::new(None);

pub(crate) fn get_current_env<'local>() -> JNIEnv<'local> {
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

/// JNI_OnLoad
///
/// jint JNI_OnLoad(JavaVM *vm, void *reserved);
///
/// The VM calls JNI_OnLoad when the native library is loaded (for example,
/// through System.loadLibrary). JNI_OnLoad must return the JNI version
/// needed by the native library.
/// In order to use any of the new JNI functions, a native library must
/// export a JNI_OnLoad function that returns JNI_VERSION_1_2. If the
/// native library does not export a JNI_OnLoad function, the VM assumes
/// that the library only requires JNI version JNI_VERSION_1_1. If the
/// VM does not recognize the version number returned by JNI_OnLoad, the
/// VM will unload the library and act as if the library was +never
/// loaded.
///
/// JNI_Onload_L(JavaVM *vm, void *reserved);
///
/// If a library L is statically linked, then upon the first invocation of
/// System.loadLibrary("L") or equivalent API, a JNI_OnLoad_L function will
/// be invoked with the same arguments and expected return value as
/// specified for the JNI_OnLoad function. JNI_OnLoad_L must return the
/// JNI version needed by the native library. This version must be
/// JNI_VERSION_1_8 or later. If the VM does not recognize the version
/// number returned by JNI_OnLoad_L, the VM will act as if the library
/// was never loaded.
///
/// LINKAGE:
/// Exported from native libraries that contain native method
/// implementation.
#[no_mangle]
#[allow(non_snake_case)]
extern "system" fn JNI_OnLoad(vm: JavaVM, _: *mut c_void) -> i32 {
    logger::init_with_android(log::LevelFilter::Info);
    hylarana_transport::startup();
    JVM.lock().unwrap().replace(vm);

    JNI_VERSION_1_6
}

/// JNI_OnUnload
///
/// void JNI_OnUnload(JavaVM *vm, void *reserved);
///
/// The VM calls JNI_OnUnload when the class loader containing the native
/// library is garbage collected. This function can be used to perform
/// cleanup operations. Because this function is called in an unknown
/// context (such as from a finalizer), the programmer should be
/// conservative on using Java VM services, and refrain from arbitrary
/// Java call-backs. Note that JNI_OnLoad and JNI_OnUnload are two
/// functions optionally supplied by JNI libraries, not exported from
/// the VM.
///
/// JNI_OnUnload_L(JavaVM *vm, void *reserved);
///
/// When the class loader containing a statically linked native library L is
/// garbage collected, the VM will invoke the JNI_OnUnload_L function of the
/// library if such a function is exported. This function can be used to
/// perform cleanup operations. Because this function is called in an
/// unknown context (such as from a finalizer), the programmer should be
/// conservative on using Java VM services, and refrain from arbitrary
/// Java call-backs.
///
/// Informational Note:
/// The act of loading a native library is the complete process of making
/// the library and its native entry points known and registered to the
/// Java VM and runtime. Note that simply performing operating system
/// level operations to load a native library, such as dlopen on a
/// UNIX(R) system, does not fully accomplish this goal. A native
/// function is normally called from the Java class loader to perform a
/// call to the host operating system that will load the library into
/// memory and return a handle to the native library. This handle will
/// be stored and used in subsequent searches for native library
/// entry points. The Java native class loader will complete the load
/// process once the handle is successfully returned to register the
/// library.
///
/// LINKAGE:
/// Exported from native libraries that contain native method
/// implementation.
#[no_mangle]
#[allow(non_snake_case)]
extern "system" fn JNI_OnUnload(_: JavaVM, _: *mut c_void) {
    hylarana_transport::shutdown();
}

fn ok_or_check<'a, F, T>(env: &mut JNIEnv<'a>, func: F) -> Option<T>
where
    F: FnOnce(&mut JNIEnv<'a>) -> Result<T>,
{
    match func(env) {
        Ok(ret) => Some(ret),
        Err(e) => {
            log::error!("java runtime exception, err={:?}", e);
            None
        }
    }
}

/// Creates the sender, the return value indicates whether the creation was
/// successful or not.
///
/// ```kt
/// private external fun createTransportSender(
///     options: TransportDescriptor,
/// ): Long
/// ```
#[no_mangle]
#[allow(non_snake_case)]
extern "system" fn Java_com_github_mycrl_hylarana_Hylarana_createTransportSender(
    mut env: JNIEnv,
    _this: JClass,
    options: JObject,
) -> *const Sender {
    ok_or_check(&mut env, |env| {
        Ok(Box::into_raw(Box::new(Sender::new(env, &options)?)))
    })
    .unwrap_or_else(|| null_mut())
}

/// get transport sender id.
///
/// ```kt
/// private external fun getTransportSenderId(
///     sender: Long
/// ): String
/// ```
#[no_mangle]
#[allow(non_snake_case)]
extern "system" fn Java_com_github_mycrl_hylarana_Hylarana_getTransportSenderId<'a>(
    mut env: JNIEnv<'a>,
    _this: JClass,
    sender: *const Sender,
) -> JString<'a> {
    assert!(!sender.is_null());

    ok_or_check(&mut env, |env| {
        Ok(env.new_string(unsafe { &*sender }.get_id())?)
    })
    .unwrap()
}

/// Sends the packet to the sender instance.
///
/// ```kt
/// private external fun sendStreamBufferToTransportSender(
///     sender: Long,
///     info: StreamBufferInfo,
///     buf: ByteArray,
/// ): Boolean
/// ```
#[no_mangle]
#[allow(non_snake_case)]
extern "system" fn Java_com_github_mycrl_hylarana_Hylarana_sendStreamBufferToTransportSender(
    mut env: JNIEnv,
    _this: JClass,
    sender: *const Sender,
    info: JObject,
    buf: JByteArray,
) -> bool {
    assert!(!sender.is_null());

    ok_or_check(&mut env, |mut env| {
        unsafe { &*sender }.sink(&mut env, info, buf)
    })
    .unwrap_or(false)
}

/// release transport sender.
///
/// ```kt
/// private external fun releaseTransportSender(sender: Long)
/// ```
#[no_mangle]
#[allow(non_snake_case)]
extern "system" fn Java_com_github_mycrl_hylarana_Hylarana_releaseTransportSender(
    _env: JNIEnv,
    _this: JClass,
    sender: *mut Sender,
) {
    assert!(!sender.is_null());

    drop(unsafe { Box::from_raw(sender) });
}

/// Creates the receiver, the return value indicates whether the creation was
/// successful or not.
///
/// ```kt
/// private external fun createTransportReceiver(
///     id: String,
///     options: TransportDescriptor,
///     observer: HylaranaReceiverAdapterObserver,
/// ): Long
/// ```
#[no_mangle]
#[allow(non_snake_case)]
extern "system" fn Java_com_github_mycrl_hylarana_Hylarana_createTransportReceiver(
    mut env: JNIEnv,
    _this: JClass,
    id: JString,
    options: JObject,
    observer: JObject,
) -> *const Arc<Receiver> {
    ok_or_check(&mut env, |env| {
        let receiver = Arc::new(Receiver::new(env, &id, &options, &observer)?);

        let adapter = receiver.get_adapter();
        let receiver_ = Arc::downgrade(&receiver);
        thread::Builder::new()
            .name("HylaranaJniStreamReceiverThread".to_string())
            .spawn(move || {
                while let Some(receiver) = receiver_.upgrade() {
                    if let Some((buf, kind, flags, timestamp)) = adapter.next() {
                        if receiver.sink(buf, kind, flags, timestamp).is_err() {
                            break;
                        }
                    } else {
                        break;
                    }
                }

                log::info!("HylaranaJniStreamReceiverThread is closed");

                if let Some(receiver) = receiver_.upgrade() {
                    let _ = receiver.close();
                }
            })?;

        Ok(Box::into_raw(Box::new(receiver)))
    })
    .unwrap_or_else(|| null_mut())
}

/// release transport receiver.
///
/// ```kt
/// private external fun releaseTransportReceiver(sender: Long)
/// ```
#[no_mangle]
#[allow(non_snake_case)]
extern "system" fn Java_com_github_mycrl_hylarana_Hylarana_releaseTransportReceiver(
    _env: JNIEnv,
    _this: JClass,
    receiver: *mut Arc<Receiver>,
) {
    assert!(!receiver.is_null());

    let _ = unsafe { Box::from_raw(receiver) }.close();
}

/// Register the service, the service type is fixed, you can customize the
/// port number, id is the identifying information of the service, used to
/// distinguish between different publishers, in properties you can add
/// customized data to the published service.
#[no_mangle]
#[allow(non_snake_case)]
extern "system" fn Java_com_github_mycrl_hylarana_Discovery_registerDiscoveryService(
    mut env: JNIEnv,
    _this: JClass,
    port: jint,
    properties: JObject,
) -> *const DiscoveryService {
    ok_or_check(&mut env, |env| {
        let properties = HashMap::<String, String>::from_map(env, &properties)?;

        Ok(Box::into_raw(Box::new(DiscoveryService::register(
            port as u16,
            &properties,
        )?)))
    })
    .unwrap_or_else(|| null_mut())
}

/// Query the registered service, the service type is fixed, when the query
/// is published the callback function will call back all the network
/// addresses of the service publisher as well as the attribute information.
#[no_mangle]
#[allow(non_snake_case)]
extern "system" fn Java_com_github_mycrl_hylarana_Discovery_queryDiscoveryService(
    mut env: JNIEnv,
    _this: JClass,
    observer: JObject,
) -> *const DiscoveryService {
    ok_or_check(&mut env, |env| {
        let observer = DiscoveryServiceObserver(env.new_global_ref(observer)?);

        Ok(Box::into_raw(Box::new(DiscoveryService::query(
            move |addrs, properties: HashMap<String, String>| {
                if let Err(e) = observer.resolve(&addrs, &properties) {
                    log::warn!("{:?}", e);
                }
            },
        )?)))
    })
    .unwrap_or_else(|| null_mut())
}

/// release the discovery service
///
/// ```kt
/// private external fun releaseDiscoveryService(discovery: Long)
/// ```
#[no_mangle]
#[allow(non_snake_case)]
extern "system" fn Java_com_github_mycrl_hylarana_Discovery_releaseDiscoveryService(
    _env: JNIEnv,
    _this: JClass,
    discovery: *mut DiscoveryService,
) {
    assert!(!discovery.is_null());

    drop(unsafe { Box::from_raw(discovery) });
}
