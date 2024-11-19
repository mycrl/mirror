use std::sync::{atomic::AtomicBool, Arc};

use anyhow::{anyhow, Result};
use bytes::Bytes;
use hylarana_common::atomic::EasyAtomic;
use hylarana_transport::{
    create_mix_receiver, StreamKind, StreamReceiverAdapter, TransportDescriptor, TransportReceiver,
};

use jni::{
    objects::{GlobalRef, JObject, JString, JValue, JValueGen},
    JNIEnv,
};

use super::{get_current_env, object::TransformObject};

pub struct Receiver {
    closed: AtomicBool,
    observer: GlobalRef,
    receiver: TransportReceiver<StreamReceiverAdapter>,
}

/// Data Stream Receiver Adapter
///  
/// Used to receive data streams from the network.
///
/// ```kt
/// abstract class HylaranaReceiverAdapterObserver {
///     /**
///      * Triggered when data arrives in the network.
///      *
///      * Note: If the buffer is empty, the current network connection has been closed or suddenly interrupted.
///      */
///     abstract fun sink(kind: Int, buf: ByteArray)
///     
///     /**
///      * stream is closed.
///      */
///     abstract fun close()
/// }
/// ```
impl Receiver {
    pub fn new(
        env: &mut JNIEnv,
        id: &JString,
        options: &JObject,
        observer: &JObject,
    ) -> Result<Self> {
        let id: String = env.get_string(id)?.into();

        Ok(Self {
            closed: AtomicBool::new(false),
            observer: env.new_global_ref(observer)?,
            receiver: create_mix_receiver(id, TransportDescriptor::from_object(env, &options)?)?,
        })
    }

    pub fn sink(&self, buf: Bytes, kind: StreamKind, flags: i32, timestamp: u64) -> Result<()> {
        let mut env = get_current_env();
        let buf = env.byte_array_from_slice(&buf)?.into();
        let ret = env.call_method(
            self.observer.as_obj(),
            "sink",
            "(IIJ[B)Z",
            &[
                JValue::Int(kind as i32),
                JValue::Int(flags),
                JValue::Long(timestamp as i64),
                JValue::Object(&buf),
            ],
        );

        let _ = env.delete_local_ref(buf);
        if let JValueGen::Bool(ret) = ret? {
            if ret == 0 {
                return Err(anyhow!("sink return false."));
            }
        } else {
            return Err(anyhow!("connect return result type missing."));
        };

        Ok(())
    }

    pub fn close(&self) -> Result<()> {
        if !self.closed.get() {
            let mut env = get_current_env();
            env.call_method(self.observer.as_obj(), "close", "()V", &[])?;
        }

        Ok(())
    }

    pub fn get_adapter(&self) -> Arc<StreamReceiverAdapter> {
        self.receiver.get_adapter()
    }
}
