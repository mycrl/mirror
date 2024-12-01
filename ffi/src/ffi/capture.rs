use std::{
    ffi::{c_char, CString},
    mem::ManuallyDrop,
};

use hylarana::{Capture, Source, SourceType};
use hylarana_common::strings::PSTR;

#[repr(C)]
#[derive(Debug, Clone, Copy)]
enum RawSourceType {
    Camera,
    Screen,
    Audio,
}

impl Into<SourceType> for RawSourceType {
    fn into(self) -> SourceType {
        match self {
            Self::Screen => SourceType::Screen,
            Self::Camera => SourceType::Camera,
            Self::Audio => SourceType::Audio,
        }
    }
}

impl From<hylarana::SourceType> for RawSourceType {
    fn from(value: SourceType) -> Self {
        match value {
            SourceType::Screen => Self::Screen,
            SourceType::Camera => Self::Camera,
            SourceType::Audio => Self::Audio,
        }
    }
}

#[repr(C)]
pub(crate) struct RawSource {
    index: usize,
    kind: RawSourceType,
    id: *const c_char,
    name: *const c_char,
    is_default: bool,
}

impl TryInto<Source> for &RawSource {
    type Error = anyhow::Error;

    fn try_into(self) -> Result<Source, Self::Error> {
        Ok(Source {
            name: PSTR::from(self.name).to_string()?,
            id: PSTR::from(self.id).to_string()?,
            is_default: self.is_default,
            kind: self.kind.into(),
            index: self.index,
        })
    }
}

#[repr(C)]
struct RawSources {
    items: *mut RawSource,
    capacity: usize,
    size: usize,
}

/// Get capture sources from sender.
#[no_mangle]
extern "C" fn hylarana_get_sources(kind: RawSourceType) -> RawSources {
    log::info!("extern api: hylarana get sources: kind={:?}", kind);

    let mut items = ManuallyDrop::new(
        Capture::get_sources(kind.into())
            .unwrap_or_else(|_| Vec::new())
            .into_iter()
            .map(|item| {
                log::info!("source: {:?}", item);

                RawSource {
                    index: item.index,
                    is_default: item.is_default,
                    kind: RawSourceType::from(item.kind),
                    id: CString::new(item.id).unwrap().into_raw(),
                    name: CString::new(item.name).unwrap().into_raw(),
                }
            })
            .collect::<Vec<RawSource>>(),
    );

    RawSources {
        items: items.as_mut_ptr(),
        capacity: items.capacity(),
        size: items.len(),
    }
}

/// Because `Sources` are allocated internally, they also need to be
/// released internally.
#[no_mangle]
extern "C" fn hylarana_sources_destroy(sources: *const RawSources) {
    assert!(!sources.is_null());

    let sources = unsafe { &*sources };
    for item in unsafe { Vec::from_raw_parts(sources.items, sources.size, sources.capacity) } {
        drop(unsafe { CString::from_raw(item.id as *mut _) });
        drop(unsafe { CString::from_raw(item.name as *mut _) });
    }
}
