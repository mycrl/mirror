use std::time::SystemTime;

pub fn checked_license() {
    let start: u64 = env!("COMPILE_TIME").parse().unwrap();
    let now = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    if now > start + (60 * 60 * 24 * 5) {
        panic!("The license has expired!")
    }
}
