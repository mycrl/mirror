pub mod camera;
pub mod screen;

use super::Source;
use crate::{Size, SourceType};

use std::{
    mem::ManuallyDrop,
    ptr::null_mut,
    slice::from_raw_parts,
    sync::{mpsc::channel, Arc},
    thread,
};

use anyhow::{anyhow, Result};
use windows::{
    core::{Interface, GUID, HSTRING, PCWSTR, PWSTR},
    Win32::{
        Graphics::Direct3D11::ID3D11Texture2D,
        Media::MediaFoundation::{
            CLSID_VideoProcessorMFT, IMF2DBuffer, IMFActivate, IMFAttributes, IMFMediaBuffer,
            IMFMediaEvent, IMFMediaEventGenerator, IMFMediaType, IMFSample, IMFSourceReader,
            IMFTransform, METransformHaveOutput, METransformNeedInput, MFCreateAttributes,
            MFCreateDXGISurfaceBuffer, MFCreateMediaType, MFCreateMemoryBuffer, MFCreateSample,
            MFEnumDeviceSources, MFMediaType_Video, MFShutdown, MFStartup, MFVideoFormat_NV12,
            MFVideoFormat_RGB32, MFVideoInterlace_Progressive,
            MEDIA_EVENT_GENERATOR_GET_EVENT_FLAGS, MFT_MESSAGE_NOTIFY_BEGIN_STREAMING,
            MFT_MESSAGE_NOTIFY_END_OF_STREAM, MFT_OUTPUT_DATA_BUFFER,
            MF_DEVSOURCE_ATTRIBUTE_FRIENDLY_NAME, MF_DEVSOURCE_ATTRIBUTE_SOURCE_TYPE,
            MF_DEVSOURCE_ATTRIBUTE_SOURCE_TYPE_AUDCAP_GUID,
            MF_DEVSOURCE_ATTRIBUTE_SOURCE_TYPE_AUDCAP_SYMBOLIC_LINK,
            MF_DEVSOURCE_ATTRIBUTE_SOURCE_TYPE_VIDCAP_GUID,
            MF_DEVSOURCE_ATTRIBUTE_SOURCE_TYPE_VIDCAP_SYMBOLIC_LINK,
            MF_SOURCE_READER_FIRST_VIDEO_STREAM, MF_TRANSFORM_ASYNC_UNLOCK, MF_VERSION,
        },
    },
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MediaFoundationSourceType {
    Video,
    Audio,
}

impl MediaFoundationSourceType {
    pub fn link_type(self) -> GUID {
        match self {
            Self::Video => MF_DEVSOURCE_ATTRIBUTE_SOURCE_TYPE_VIDCAP_SYMBOLIC_LINK,
            Self::Audio => MF_DEVSOURCE_ATTRIBUTE_SOURCE_TYPE_AUDCAP_SYMBOLIC_LINK,
        }
    }
}

impl Into<GUID> for MediaFoundationSourceType {
    fn into(self) -> GUID {
        match self {
            Self::Video => MF_DEVSOURCE_ATTRIBUTE_SOURCE_TYPE_VIDCAP_GUID,
            Self::Audio => MF_DEVSOURCE_ATTRIBUTE_SOURCE_TYPE_AUDCAP_GUID,
        }
    }
}

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

    fn set_values<T: IntoIterator<Item = (GUID, IMFValue)>>(
        &mut self,
        values: T,
    ) -> Result<(), anyhow::Error> {
        for (k, v) in values.into_iter() {
            self.set(k, v)?;
        }

        Ok(())
    }
}

impl MediaFoundationIMFAttributesSetHelper for IMFAttributes {}

impl AsIMFAttributes for IMFAttributes {
    fn as_attributes(&self) -> &IMFAttributes {
        &self
    }
}

impl MediaFoundationIMFAttributesSetHelper for IMFActivate {}

impl AsIMFAttributes for IMFActivate {
    fn as_attributes(&self) -> &IMFAttributes {
        &self
    }
}

impl MediaFoundationIMFAttributesSetHelper for IMFMediaType {}

impl AsIMFAttributes for IMFMediaType {
    fn as_attributes(&self) -> &IMFAttributes {
        &self
    }
}

pub trait SampleIterator {
    type Item;

    fn next(&mut self) -> Result<Option<Self::Item>, anyhow::Error>;
}

impl SampleIterator for IMFSourceReader {
    type Item = IMFSample;

    fn next(&mut self) -> Result<Option<Self::Item>, anyhow::Error> {
        let mut sample = None;
        let mut index = 0;
        let mut flags = 0;
        let mut timestamp = 0;
        unsafe {
            self.ReadSample(
                MF_SOURCE_READER_FIRST_VIDEO_STREAM.0 as u32,
                0,
                Some(&mut index),
                Some(&mut flags),
                Some(&mut timestamp),
                Some(&mut sample),
            )?;
        }

        Ok(if index != 0 { None } else { sample })
    }
}

pub struct MediaFoundation;

impl MediaFoundation {
    /// Initializes Microsoft Media Foundation.
    pub fn startup() -> Result<()> {
        unsafe { MFStartup(MF_VERSION, 0) }?;
        Ok(())
    }

    /// Shuts down the Microsoft Media Foundation platform. Call this function
    /// once for every call to MFStartup. Do not call this function from work
    /// queue threads.
    pub fn shutdown() -> Result<()> {
        unsafe { MFShutdown() }?;
        Ok(())
    }

    /// Creates an empty attribute store.
    pub fn create_attributes() -> Result<IMFAttributes> {
        let mut attributes = None;
        unsafe { MFCreateAttributes(&mut attributes, 1) }?;
        let attributes = attributes.ok_or_else(|| anyhow!("failed to create imf attributes"))?;
        Ok(attributes)
    }

    pub fn get_sources(kind: MediaFoundationSourceType) -> Result<Vec<Source>> {
        let mut attributes = Self::create_attributes()?;
        attributes.set(
            MF_DEVSOURCE_ATTRIBUTE_SOURCE_TYPE,
            IMFValue::GUID(kind.into()),
        )?;

        // Enumerates a list of audio or video capture devices.
        let mut count = 0;
        let mut activates = null_mut();
        unsafe {
            MFEnumDeviceSources(&attributes, &mut activates, &mut count)?;
        };

        if activates.is_null() {
            return Err(anyhow!("devices is empty"));
        }

        let mut sources = Vec::with_capacity(count as usize);
        for item in unsafe { from_raw_parts(activates, count as usize) } {
            if let Some(activate) = item {
                if let (Some(name), Some(id)) = (
                    activate.get_string(MF_DEVSOURCE_ATTRIBUTE_FRIENDLY_NAME),
                    activate.get_string(kind.link_type()),
                ) {
                    sources.push(Source {
                        id,
                        name,
                        index: sources.len(),
                        kind: match kind {
                            MediaFoundationSourceType::Video => SourceType::Camera,
                            MediaFoundationSourceType::Audio => SourceType::Audio,
                        },
                    });
                }
            }
        }

        Ok(sources)
    }
}
