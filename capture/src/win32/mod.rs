pub mod camera;
pub mod screen;

use anyhow::Result;
use windows::{
    core::{GUID, HSTRING, PCWSTR, PWSTR},
    Win32::{
        Media::MediaFoundation::{
            IMFActivate, IMFAttributes, IMFMediaType, MFShutdown, MFStartup, MF_VERSION,
        },
        System::Com::{CoInitializeEx, CoUninitialize, COINIT_MULTITHREADED},
    },
};

#[allow(unused)]
pub enum IMFValue {
    GUID(GUID),
    String(String),
    U32(u32),
    U64(u64),
    DoubleU32(u32, u32),
}

pub trait AsIMFAttributes {
    fn as_attributes(&self) -> &IMFAttributes;
}

pub trait MediaFoundationIMFAttributesSetHelper: AsIMFAttributes {
    fn get_string(&self, key: GUID) -> Option<String> {
        // Gets a wide-character string associated with a key. This method allocates the
        // memory for the string.
        let mut size = 0;
        let mut pwstr = PWSTR::null();
        unsafe {
            self.as_attributes()
                .GetAllocatedString(&key, &mut pwstr, &mut size)
                .ok()?;
        }

        if !pwstr.is_null() {
            Some(unsafe { pwstr.to_string().ok()? })
        } else {
            None
        }
    }

    fn set(&mut self, key: GUID, value: IMFValue) -> Result<(), anyhow::Error> {
        let attr = self.as_attributes();
        unsafe {
            match value {
                IMFValue::U32(v) => attr.SetUINT32(&key, v)?,
                IMFValue::U64(v) => attr.SetUINT64(&key, v)?,
                IMFValue::GUID(v) => attr.SetGUID(&key, &v)?,
                IMFValue::String(v) => attr.SetString(&key, PCWSTR(HSTRING::from(v).as_ptr()))?,
                IMFValue::DoubleU32(x, v) => attr.SetUINT64(&key, ((x as u64) << 32) | v as u64)?,
            }
        }

        Ok(())
    }
}

impl MediaFoundationIMFAttributesSetHelper for IMFAttributes {}

impl AsIMFAttributes for IMFAttributes {
    fn as_attributes(&self) -> &IMFAttributes {
        self
    }
}

impl MediaFoundationIMFAttributesSetHelper for IMFActivate {}

impl AsIMFAttributes for IMFActivate {
    fn as_attributes(&self) -> &IMFAttributes {
        self
    }
}

impl MediaFoundationIMFAttributesSetHelper for IMFMediaType {}

impl AsIMFAttributes for IMFMediaType {
    fn as_attributes(&self) -> &IMFAttributes {
        self
    }
}

/// Initializes Microsoft Media Foundation.
pub fn startup() -> Result<()> {
    unsafe {
        CoInitializeEx(None, COINIT_MULTITHREADED).ok()?;
        MFStartup(MF_VERSION, 0)?;
    }

    Ok(())
}

/// Shuts down the Microsoft Media Foundation platform. Call this function
/// once for every call to MFStartup. Do not call this function from work
/// queue threads.
pub fn shutdown() -> Result<()> {
    unsafe {
        MFShutdown()?;
        CoUninitialize();
    }

    Ok(())
}
