use devices::{init, DeviceConstraint, Devices};

fn main() {
    init();

    let devices = Devices::get_video_devices();
    for device in &devices {
        println!("{:?}", device.description());
    }

    if !devices.is_empty() {
        let device = devices[1].open(DeviceConstraint {
            width: 1920,
            height: 1080,
            frame_rate: 30,
        }).unwrap();
        loop {
            if device.make_readable() {
                while let Some(frame) = device.get_frame() {
                    println!("frame: width={}, height={}, format={}", frame.width, frame.height, frame.format)
                }
            } else {
                break;
            }
        }
    }
}
