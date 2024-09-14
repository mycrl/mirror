use anyhow::anyhow;
use bytes::Bytes;
use jni::objects::{GlobalRef, JValue, JValueGen};
use transport::adapter::StreamKind;

use super::common::{catcher, get_current_env};

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
    pub(crate) fn sink(&self, buf: Bytes, kind: StreamKind, flags: i32, timestamp: u64) -> bool {
        let mut env = get_current_env();
        catcher(&mut env, |env| {
            let buf = env.byte_array_from_slice(&buf)?.into();
            let ret = env.call_method(
                self.callback.as_obj(),
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
