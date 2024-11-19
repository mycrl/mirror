use std::{
    collections::HashMap,
    net::{Ipv4Addr, SocketAddr},
};

use anyhow::{anyhow, Result};
use hylarana_transport::{StreamBufferInfo, StreamKind, TransportDescriptor, TransportStrategy};
use jni::{
    objects::{JMap, JObject, JObjectArray, JString, JValueGen},
    JNIEnv,
};

pub trait TransformObject: Sized {
    #[allow(unused)]
    fn from_object(env: &mut JNIEnv, object: &JObject) -> Result<Self> {
        unimplemented!()
    }
}

pub trait TransformArray: Sized {
    #[allow(unused)]
    fn to_array<'a>(&self, env: &mut JNIEnv<'a>) -> Result<JObjectArray<'a>> {
        unimplemented!()
    }
}

pub trait TransformMap: Sized {
    #[allow(unused)]
    fn from_map(env: &mut JNIEnv, map: &JObject) -> Result<Self> {
        unimplemented!()
    }

    #[allow(unused)]
    fn to_map<'a>(&self, env: &mut JNIEnv<'a>) -> Result<JObject<'a>> {
        unimplemented!()
    }
}

pub trait EasyObject<'a>
where
    Self: AsRef<JObject<'a>>,
{
    fn get_string(&self, env: &mut JNIEnv, key: &str) -> Result<String> {
        if let JValueGen::Object(value) = env.get_field(&self, key, "Ljava/lang/String;")? {
            Ok(env.get_string(&JString::from(value))?.into())
        } else {
            Err(anyhow!("[{}] not a string", key))
        }
    }

    fn get_int(&self, env: &mut JNIEnv, key: &str) -> Result<i32> {
        if let JValueGen::Int(value) = env.get_field(&self, key, "I")? {
            Ok(value)
        } else {
            Err(anyhow!("[{}] not a int", key))
        }
    }

    fn get_long(&self, env: &mut JNIEnv, key: &str) -> Result<i64> {
        if let JValueGen::Long(value) = env.get_field(&self, key, "J")? {
            Ok(value)
        } else {
            Err(anyhow!("[{}] not a long", key))
        }
    }

    fn get_object<'b>(&self, env: &mut JNIEnv<'b>, ty: &str, key: &str) -> Result<JObject<'b>> {
        if let JValueGen::Object(value) = env.get_field(&self, key, ty)? {
            Ok(value)
        } else {
            Err(anyhow!("[{}] not a object", key))
        }
    }
}

impl<'a> EasyObject<'a> for JObject<'a> {}

// ```kt
// /**
//  * transport strategy
//  */
// data class TransportStrategy(
//     /**
//      * STRATEGY_DIRECT | STRATEGY_RELAY | STRATEGY_MULTICAST
//      */
//     val type: Int,
//     /**
//      * socket address
//      */
//     val addr: String
// )
// ```
impl TransformObject for TransportStrategy {
    fn from_object(env: &mut JNIEnv, object: &JObject) -> Result<Self> {
        let addr: SocketAddr = object.get_string(env, "addr")?.parse()?;

        Ok(match object.get_int(env, "type")? {
            0 => Self::Direct(addr),
            1 => Self::Relay(addr),
            2 => Self::Multicast(addr),
            _ => return Err(anyhow!("type of invalidity")),
        })
    }
}

// ```kt
// data class TransportDescriptor(
//     val strategy: TransportStrategy,
//     /**
//      * see: [Maximum_transmission_unit](https://en.wikipedia.org/wiki/Maximum_transmission_unit)
//      */
//     val mtu: Int
// )
// ```
impl TransformObject for TransportDescriptor {
    fn from_object(env: &mut JNIEnv, object: &JObject) -> Result<Self> {
        let strategy = object.get_object(
            env,
            "Lcom/github/mycrl/hylarana/TransportStrategy;",
            "strategy",
        )?;

        Ok(Self {
            strategy: TransportStrategy::from_object(env, &strategy)?,
            mtu: object.get_int(env, "mtu")? as usize,
        })
    }
}

// ```kt
// /**
//  * STREAM_TYPE_VIDEO | STREAM_TYPE_AUDIO
//  */
// data class StreamBufferInfo(val type: Int) {
//     var flags: Int = 0
//     var timestamp: Long = 0
// }
// ```
impl TransformObject for StreamBufferInfo {
    fn from_object(env: &mut JNIEnv, object: &JObject) -> Result<Self> {
        let flags = object.get_int(env, "flags")?;
        let timestamp = object.get_long(env, "timestamp")? as u64;

        Ok(
            match StreamKind::try_from(object.get_int(env, "type")? as u8)
                .map_err(|_| anyhow!("type unreachable"))?
            {
                StreamKind::Video => Self::Video(flags, timestamp),
                StreamKind::Audio => Self::Audio(flags, timestamp),
            },
        )
    }
}

// ```kt
// typealias Properties = Map<String, String>;
// ```
impl TransformMap for HashMap<String, String> {
    fn from_map(env: &mut JNIEnv, object: &JObject) -> Result<Self> {
        let mut map = HashMap::with_capacity(10);

        let jmap = JMap::from_env(env, &object)?;
        let mut iterator = jmap.iter(env)?;
        while let Some((key, value)) = iterator.next(env)? {
            map.insert(
                env.get_string(&JString::from(key))?.into(),
                env.get_string(&JString::from(value))?.into(),
            );
        }

        Ok(map)
    }

    fn to_map<'a>(&self, env: &mut JNIEnv<'a>) -> Result<JObject<'a>> {
        let object = env.new_object("java/util/HashMap", "()V", &[])?;
        let map = JMap::from_env(env, &object)?;

        for (key, value) in self.iter() {
            map.put(
                env,
                env.new_string(key)?.as_ref(),
                env.new_string(value)?.as_ref(),
            )?;
        }

        Ok(object)
    }
}

// ```kt
// Array<String>
// ```
impl TransformArray for Vec<Ipv4Addr> {
    fn to_array<'a>(&self, env: &mut JNIEnv<'a>) -> Result<JObjectArray<'a>> {
        let array =
            env.new_object_array(self.len() as i32, "java/lang/String", JString::default())?;

        for (i, item) in self.iter().enumerate() {
            env.set_object_array_element(&array, i as i32, env.new_string(&item.to_string())?)?;
        }

        Ok(array)
    }
}
