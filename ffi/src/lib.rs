// #[cfg(target_os = "android")]
mod jni;

#[cfg(not(target_os = "android"))]
mod ffi;
