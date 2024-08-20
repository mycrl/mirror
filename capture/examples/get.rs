use eye_hal::{
    format::PixelFormat,
    traits::{Context, Device},
    PlatformContext,
};

fn main() {
    let ctx = PlatformContext::default();
    let devices = ctx.devices().unwrap();
    println!("{:#?}", devices);

    let device = ctx.open_device(&devices[0].uri).unwrap();
    let streams = device.streams().unwrap();

    for it in streams.iter().filter(|it| {
        it.pixfmt == PixelFormat::Custom("YUYV".to_string())
            || it.pixfmt == PixelFormat::Custom("NV12".to_string())
    }) {
        println!("{:?}", it);
    }
}
