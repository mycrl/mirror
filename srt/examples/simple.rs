use std::{io::Error, thread, time::Duration};

use rand::prelude::*;
use srt::{FragmentDecoder, FragmentEncoder, Options, Server, Socket};

fn main() -> Result<(), Error> {
    srt::startup();

    let mut opt = Options::default();
    opt.latency = 20;
    opt.mtu = 1400;
    opt.fc = 32;

    let mut rng = rand::thread_rng();
    let mut bytes = vec![0u8; rng.gen_range(100..=10000)];
    rng.fill_bytes(&mut bytes);

    let opt_ = opt.clone();
    let bytes_ = bytes.clone();
    thread::spawn(move || {
        let mut server = Server::bind("0.0.0.0:8088".parse().unwrap(), opt_, 100).unwrap();

        while let Ok((socket, addr)) = server.accept() {
            let bytes = bytes_.clone();

            println!("new socket = {}", addr);

            thread::spawn(move || {
                let mut buf = [0u8; 2000];
                let mut decoder = FragmentDecoder::new();

                loop {
                    let size = socket.read(&mut buf).unwrap();
                    if size == 0 {
                        break;
                    }

                    if let Some((_, payload)) = decoder.decode(&buf[..size]) {
                        println!("{} = {}", payload.len(), bytes.len());
                        assert_eq!(&payload, &bytes);
                    }
                }
            });
        }
    });

    let socket = Socket::connect("127.0.0.1:8088".parse().unwrap(), opt.clone())?;
    let mut encoder = FragmentEncoder::new(opt.max_pkt_size());

    'a: loop {
        for chunk in encoder.encode(&bytes) {
            if socket.send(chunk).is_err() {
                break 'a;
            }
        }

        thread::sleep(Duration::from_secs(1))
    }

    srt::cleanup();
    Ok(())
}
