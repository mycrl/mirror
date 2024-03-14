use devices::{init, Devices};

fn main() {
    init();
    
    let devices = Devices::get_video_devices();
    if !devices.is_empty() {
        let device = devices[0].open().unwrap();
        loop {
            println!("{:?}", device.next().is_none());
        }
    }
}
