use std::{
    ffi::{c_char, CStr, CString},
    ptr,
    str::Utf8Error,
};

use thiserror::Error;

#[derive(Debug, Error)]
pub enum StringError {
    #[error(transparent)]
    Utf8Error(#[from] Utf8Error),
    #[error("the string ptr is null")]
    Null,
}

/// A type representing an owned, C-compatible, nul-terminated string with no
/// nul bytes in the middle.
///
/// This type serves the purpose of being able to safely generate a C-compatible
/// string from a Rust byte slice or vector. An instance of this type is a
/// static guarantee that the underlying bytes contain no interior 0 bytes (“nul
/// characters”) and that the final byte is 0 (“nul terminator”).
///
/// CString is to &CStr as String is to &str: the former in each pair are owned
/// strings; the latter are borrowed references.
pub struct PSTR {
    ptr: *const c_char,
    drop: bool,
}

impl From<*const c_char> for PSTR {
    fn from(ptr: *const c_char) -> Self {
        Self { drop: false, ptr }
    }
}

impl From<&str> for PSTR {
    fn from(value: &str) -> Self {
        Self {
            ptr: CString::new(value).unwrap().into_raw(),
            drop: true,
        }
    }
}

impl From<String> for PSTR {
    fn from(value: String) -> Self {
        Self {
            ptr: CString::new(value).unwrap().into_raw(),
            drop: true,
        }
    }
}

impl PSTR {
    /// Yields a &str slice if the CStr contains valid UTF-8.
    ///
    /// If the contents of the CStr are valid UTF-8 data, this function will
    /// return the corresponding &str slice. Otherwise, it will return an error
    /// with details of where UTF-8 validation failed.
    pub fn to_string(&self) -> Result<String, StringError> {
        if !self.ptr.is_null() {
            Ok(unsafe { CStr::from_ptr(self.ptr) }
                .to_str()
                .map(|s| s.to_string())?)
        } else {
            Err(StringError::Null)
        }
    }

    /// Returns the inner pointer to this C string.
    ///
    ///The returned pointer will be valid for as long as self is, and points to
    /// a contiguous region of memory terminated with a 0 byte to represent the
    /// end of the string.
    ///
    ///The type of the returned pointer is *const c_char, and whether it’s an
    /// alias for *const i8 or *const u8 is platform-specific.
    ///
    /// ### WARNING
    ///
    ///The returned pointer is read-only; writing to it (including passing it
    /// to C code that writes to it) causes undefined behavior.
    pub fn as_ptr(&self) -> *const c_char {
        self.ptr
    }

    /// Copy the string pointed to by src to dest.
    ///
    /// The behavior of this function is essentially the same as the behavior of
    /// `strcpy` in the C standard library.
    pub fn strcpy(src: &str, dest: *mut c_char) {
        let src = src.as_bytes();
        let len = src.len();

        unsafe {
            ptr::copy(src.as_ptr().cast(), dest, len);
            ptr::write(dest.offset(len as isize + 1) as *mut u8, b'\0');
        }
    }
}

impl Drop for PSTR {
    fn drop(&mut self) {
        if self.drop && !self.ptr.is_null() {
            drop(unsafe { CString::from_raw(self.ptr as *mut c_char) })
        }
    }
}
