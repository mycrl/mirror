use std::sync::Arc;

use anyhow::Result;
use bytes::BytesMut;
use hylarana_transport::{
    create_sender, with_capacity as package_with_capacity, StreamBufferInfo, StreamSenderAdapter,
    TransportOptions, TransportSender,
};

use jni::{
    objects::{JByteArray, JObject},
    JNIEnv,
};

use super::object::TransformObject;

pub struct Sender {
    sender: TransportSender,
    adapter: Arc<StreamSenderAdapter>,
}

impl Sender {
    pub fn new(env: &mut JNIEnv, options: &JObject) -> Result<Self> {
        let sender = create_sender(TransportOptions::from_object(env, &options)?)?;
        Ok(Self {
            adapter: sender.get_adapter(),
            sender,
        })
    }

    pub fn get_id(&self) -> &str {
        self.sender.get_id()
    }

    pub fn sink(&self, env: &mut JNIEnv, info: JObject, buf: JByteArray) -> Result<bool> {
        let buf = copy_from_byte_array(env, &buf)?;
        let info = StreamBufferInfo::from_object(env, &info)?;
        Ok(self.adapter.send(buf, info))
    }
}

fn copy_from_byte_array(env: &JNIEnv, array: &JByteArray) -> Result<BytesMut> {
    let size = env.get_array_length(array)? as usize;
    let mut bytes = package_with_capacity(size);
    let start = bytes.len() - size;

    env.get_byte_array_region(array, 0, unsafe {
        std::mem::transmute::<&mut [u8], &mut [i8]>(&mut bytes[start..])
    })?;

    Ok(bytes)
}
