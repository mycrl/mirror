use std::{collections::HashMap, net::Ipv4Addr};

use anyhow::Result;
use jni::objects::{GlobalRef, JValue};

use super::{get_current_env, object::TransformMap, TransformArray};

pub struct DiscoveryServiceObserver(pub GlobalRef);

unsafe impl Send for DiscoveryServiceObserver {}
unsafe impl Sync for DiscoveryServiceObserver {}

impl DiscoveryServiceObserver {
    pub fn resolve(
        &self,
        addrs: &Vec<Ipv4Addr>,
        properties: &HashMap<String, String>,
    ) -> Result<()> {
        let mut env = get_current_env();
        let addrs = addrs.to_array(&mut env)?;
        let properties = properties.to_map(&mut env).unwrap();
        env.call_method(
            self.0.as_obj(),
            "resolve",
            "([Ljava/lang/String;Ljava/util/Map;)V",
            &[
                JValue::Object(addrs.as_ref()),
                JValue::Object(properties.as_ref()),
            ],
        )?;

        Ok(())
    }
}
