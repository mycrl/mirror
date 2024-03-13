use devices::{init, Devices};

fn main() {
    init();
    println!("{:?}", Devices::get_audio_devices());
}
