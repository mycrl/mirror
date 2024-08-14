mod screen;

pub use self::screen::ScreenCapture;

pub fn startup() {
    if !scap::has_permission() {
        if !scap::request_permission() {
            log::error!("linux capture request permission failed.")
        }
    }
}

pub fn shutdown() {}
