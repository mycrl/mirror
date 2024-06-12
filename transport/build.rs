use std::time::SystemTime;

fn main() {
    println!(
        "cargo::rustc-env=COMPILE_TIME={}",
        SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_secs()
    )
}
