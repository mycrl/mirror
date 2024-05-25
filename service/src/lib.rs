pub mod route;
pub mod signal;

#[repr(u8)]
#[derive(Default, PartialEq, Eq, Debug)]
pub enum SocketKind {
    #[default]
    Subscriber = 0,
    Publisher = 1,
}

#[derive(Default, Debug)]
pub struct StreamInfo {
    pub id: u32,
    pub port: Option<u16>,
    pub kind: SocketKind,
}

impl StreamInfo {
    pub fn decode(value: &str) -> Option<Self> {
        if value.starts_with("#!::") {
            let mut info = Self::default();
            for item in value.split_at(4).1.split(",") {
                if let Some((k, v)) = item.split_once("=") {
                    match k {
                        "i" => {
                            if let Ok(id) = v.parse::<u32>() {
                                info.id = id;
                            }
                        }
                        "k" => {
                            if let Ok(kind) = v.parse::<u8>() {
                                match kind {
                                    0 => {
                                        info.kind = SocketKind::Subscriber;
                                    }
                                    1 => {
                                        info.kind = SocketKind::Publisher;
                                    }
                                    _ => (),
                                }
                            }
                        }
                        "p" => {
                            if let Ok(port) = v.parse::<u16>() {
                                info.port = Some(port);
                            }
                        }
                        _ => (),
                    }
                }
            }

            Some(info)
        } else {
            None
        }
    }

    pub fn encode(self) -> String {
        let mut format = "#!::".to_string();
        format.push_str(&format!("i={}", self.id));
        format.push_str(&format!("k={}", self.kind as u8));
        if let Some(port) = self.port {
            format.push_str(&format!("p={}", port));
        }

        format
    }
}
