use std::{
    net::SocketAddr,
    sync::{Arc, Weak},
    thread,
};

use anyhow::anyhow;
use bytes::Bytes;
use jni::objects::{GlobalRef, JValue, JValueGen};
use transport::adapter::{ReceiverAdapterFactory, StreamKind, StreamReceiverAdapter};

use crate::command::{catcher, get_current_env};

pub struct AndroidStreamReceiverAdapter {
    pub callback: GlobalRef,
}

impl AndroidStreamReceiverAdapter {
    // /**
    //  * Data Stream Receiver Adapter
    //  *
    //  * Used to receive data streams from the network.
    //  */
    // abstract class ReceiverAdapter {
    //     /**
    //      * Triggered when data arrives in the network.
    //      *
    //      * Note: If the buffer is empty, the current network connection has been
    //      * closed or suddenly interrupted.
    //      */
    //     abstract fun sink(kind: Int, buf: ByteArray)
    // }
    pub(crate) fn sink(&self, buf: Bytes, kind: StreamKind, timestamp: u64) -> bool {
        let mut env = get_current_env();
        catcher(&mut env, |env| {
            let buf = env.byte_array_from_slice(&buf)?.into();
            let ret = env.call_method(
                self.callback.as_obj(),
                "sink",
                "(IJ[B)Z",
                &[
                    JValue::Int(kind as i32),
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
        })
        .is_some()
    }

    pub(crate) fn close(&self) {
        let mut env = get_current_env();
        catcher(&mut env, |env| {
            env.call_method(self.callback.as_obj(), "close", "()V", &[])?;

            Ok(())
        });
    }
}

pub struct AndroidStreamReceiverAdapterFactory {
    pub callback: GlobalRef,
}

impl AndroidStreamReceiverAdapterFactory {
    // /**
    //  * Data Stream Receiver Adapter Factory
    //  */
    // abstract class ReceiverAdapterFactory {
    //     /**
    //      * Called when a new connection comes in.
    //      *
    //      * You can choose to return Null, which will cause the connection to be
    //        rejected.
    //      */
    //     abstract fun connect(id: Int, ip: String, description: ByteArray):
    // ReceiverAdapter? }
    fn connect(
        &self,
        id: u8,
        ip: String,
        description: &[u8],
    ) -> Option<AndroidStreamReceiverAdapter> {
        let mut env = get_current_env();
        catcher(&mut env, |env| {
            let ip = env.new_string(ip)?.into();
            let description = env.byte_array_from_slice(description)?.into();
            let ret = env.call_method(
                self.callback.as_obj(),
                "connect",
                "(ILjava/lang/String;[B)Lcom/github/mycrl/mirror/ReceiverAdapter;",
                &[
                    JValue::Int(id as i32),
                    JValue::Object(&ip),
                    JValue::Object(&description),
                ],
            );

            let _ = env.delete_local_ref(ip);
            let _ = env.delete_local_ref(description);

            let callback = if let JValueGen::Object(callback) = ret? {
                callback
            } else {
                return Err(anyhow!("connect return result type missing."));
            };

            Ok(if !callback.is_null() {
                Some(AndroidStreamReceiverAdapter {
                    callback: env.new_global_ref(callback)?,
                })
            } else {
                None
            })
        })
        .flatten()
    }
}

impl ReceiverAdapterFactory for AndroidStreamReceiverAdapterFactory {
    fn connect(
        &self,
        id: u8,
        addr: SocketAddr,
        description: &[u8],
    ) -> Option<Weak<StreamReceiverAdapter>> {
        let this = unsafe { std::mem::transmute::<&Self, &'static Self>(self) };
        let description = unsafe { std::mem::transmute::<&[u8], &'static [u8]>(description) };
        let adapter = this.connect(id, addr.to_string(), description)?;

        let stream_adapter = StreamReceiverAdapter::new();
        let stream_adapter_ = Arc::downgrade(&stream_adapter);
        thread::spawn(move || {
            while let Some((buf, kind, timestamp)) = stream_adapter.next() {
                if !adapter.sink(buf, kind, timestamp) {
                    break;
                }
            }

            adapter.close();
        });

        Some(stream_adapter_)
    }
}
