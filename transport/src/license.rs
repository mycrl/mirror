use std::time::SystemTime;

include!("../../license.rs");

pub fn checked_license() {
    if SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_secs()
        > START_TIME + TIMEOUT
    {
        panic!("The license has expired!")
    }
}
