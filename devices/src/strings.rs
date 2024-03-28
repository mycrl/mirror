use std::ffi::{c_char, CString};

pub struct Strings {
    ptr: *const c_char,
    drop: bool,
}

impl Drop for Strings {
    fn drop(&mut self) {
        if self.drop && !self.ptr.is_null() {
            drop(unsafe { CString::from_raw(self.ptr as *mut c_char) })
        }
    }
}

impl From<*const c_char> for Strings {
    fn from(ptr: *const c_char) -> Self {
        Self { drop: false, ptr }
    }
}

impl From<&str> for Strings {
    fn from(value: &str) -> Self {
        Self {
            ptr: CString::new(value).unwrap().into_raw(),
            drop: true,
        }
    }
}

impl Strings {
    pub fn to_string(&self) -> Option<String> {
        if !self.ptr.is_null() {
            unsafe { std::ffi::CStr::from_ptr(self.ptr) }
                .to_str()
                .map(|s| s.to_string())
                .ok()
        } else {
            None
        }
    }

    pub fn as_ptr(&self) -> *const c_char {
        self.ptr
    }
}
