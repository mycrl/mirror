use std::{cell::Cell, ffi::c_void};

use crate::Size;

pub use windows;

use windows::{
    core::{s, Interface, Result, GUID, HSTRING, PCSTR, PCWSTR, PWSTR},
    Win32::{
        Foundation::{HANDLE, HWND, RECT},
        Graphics::{
            Direct3D::{
                D3D_DRIVER_TYPE_HARDWARE, D3D_FEATURE_LEVEL, D3D_FEATURE_LEVEL_11_0,
                D3D_FEATURE_LEVEL_11_1,
            },
            Direct3D11::{
                D3D11CreateDevice, ID3D11Device, ID3D11DeviceContext, ID3D11Multithread,
                ID3D11Texture2D, D3D11_CREATE_DEVICE_BGRA_SUPPORT, D3D11_CREATE_DEVICE_DEBUG,
                D3D11_SDK_VERSION, D3D11_TEXTURE2D_DESC,
            },
            Dxgi::IDXGIResource,
        },
        Media::MediaFoundation::{
            IMFActivate, IMFAttributes, IMFMediaType, MFShutdown, MFStartup, MF_VERSION,
        },
        System::{
            Com::{CoInitializeEx, CoUninitialize, COINIT_MULTITHREADED},
            Threading::{
                AvRevertMmThreadCharacteristics, AvSetMmThreadCharacteristicsA, GetCurrentProcess,
                SetPriorityClass, BELOW_NORMAL_PRIORITY_CLASS, HIGH_PRIORITY_CLASS,
                NORMAL_PRIORITY_CLASS, PROCESS_CREATION_FLAGS, PROCESS_MODE_BACKGROUND_BEGIN,
                REALTIME_PRIORITY_CLASS,
            },
        },
        UI::WindowsAndMessaging::GetClientRect,
    },
};

pub fn get_hwnd_size(hwnd: HWND) -> Result<Size> {
    let mut rect = RECT::default();
    unsafe {
        GetClientRect(hwnd, &mut rect)?;
    }

    Ok(Size {
        width: (rect.right - rect.left) as u32,
        height: (rect.bottom - rect.top) as u32,
    })
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

    fn set(&mut self, key: GUID, value: IMFValue) -> Result<()> {
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

pub enum ProcessPriority {
    High,
    Low,
    Normal,
    Realtime,
    Background,
}

impl Into<PROCESS_CREATION_FLAGS> for ProcessPriority {
    fn into(self) -> PROCESS_CREATION_FLAGS {
        match self {
            Self::High => HIGH_PRIORITY_CLASS,
            Self::Low => BELOW_NORMAL_PRIORITY_CLASS,
            Self::Normal => NORMAL_PRIORITY_CLASS,
            Self::Realtime => REALTIME_PRIORITY_CLASS,
            Self::Background => PROCESS_MODE_BACKGROUND_BEGIN,
        }
    }
}

/// Sets the priority class for the specified process. This value together with
/// the priority value of each thread of the process determines each thread's
/// base priority level.
pub fn set_process_priority(priority: ProcessPriority) -> Result<()> {
    unsafe { SetPriorityClass(GetCurrentProcess(), priority.into()) }
}

pub enum MediaThreadClass {
    Audio,
    Capture,
    DisplayPostProcessing,
    Distribution,
    Games,
    Playback,
    ProAudio,
    WindowManager,
}

impl Into<PCSTR> for MediaThreadClass {
    fn into(self) -> PCSTR {
        match self {
            Self::Audio => s!("Audio"),
            Self::Capture => s!("Capture"),
            Self::DisplayPostProcessing => s!("DisplayPostProcessing"),
            Self::Distribution => s!("Distribution"),
            Self::Games => s!("Games"),
            Self::Playback => s!("Playback"),
            Self::ProAudio => s!("Pro Audio"),
            Self::WindowManager => s!("Window Manager"),
        }
    }
}

thread_local!(static THREAD_CLASS_HANDLE: Cell<Option<HANDLE>> = Cell::new(None));

pub struct MediaThreadClassGuard;

impl Drop for MediaThreadClassGuard {
    fn drop(&mut self) {
        if let Some(handle) = THREAD_CLASS_HANDLE.get() {
            if let Err(e) = unsafe { AvRevertMmThreadCharacteristics(handle) } {
                log::warn!("AvRevertMmThreadCharacteristics error={:?}", e)
            }
        }
    }
}

impl MediaThreadClass {
    pub fn join(self) -> Result<MediaThreadClassGuard> {
        let mut taskindex = 0;
        let taskname: PCSTR = self.into();
        match unsafe { AvSetMmThreadCharacteristicsA(taskname, &mut taskindex) } {
            Ok(handle) => THREAD_CLASS_HANDLE.set(Some(handle)),
            Err(e) => {
                log::warn!("AvSetMmThreadCharacteristics error={:?}", e)
            }
        }

        Ok(MediaThreadClassGuard)
    }
}

#[derive(Debug, Clone)]
pub struct Direct3DDevice {
    pub device: ID3D11Device,
    pub context: ID3D11DeviceContext,
}

impl Direct3DDevice {
    pub fn new() -> Result<Direct3DDevice> {
        unsafe {
            let (mut d3d_device, mut d3d_context, mut feature_level) =
                (None, None, D3D_FEATURE_LEVEL::default());

            D3D11CreateDevice(
                None,
                D3D_DRIVER_TYPE_HARDWARE,
                None,
                D3D11_CREATE_DEVICE_BGRA_SUPPORT | D3D11_CREATE_DEVICE_DEBUG,
                Some(&[D3D_FEATURE_LEVEL_11_1, D3D_FEATURE_LEVEL_11_0]),
                D3D11_SDK_VERSION,
                Some(&mut d3d_device),
                Some(&mut feature_level),
                Some(&mut d3d_context),
            )?;

            Ok(Direct3DDevice {
                device: d3d_device.unwrap(),
                context: d3d_context.unwrap(),
            })
        }
    }

    pub fn set_multithread_protected(&self, value: bool) -> Result<()> {
        let multithread = self.device.cast::<ID3D11Multithread>()?;
        let _ = unsafe { multithread.SetMultithreadProtected(value) };
        Ok(())
    }

    /// open shared texture.
    pub fn open_shared_texture(&self, handle: HANDLE) -> Result<ID3D11Texture2D> {
        Ok(unsafe {
            let mut texture: Option<ID3D11Texture2D> = None;
            self.device.OpenSharedResource(handle, &mut texture)?;
            texture.unwrap()
        })
    }
}

#[inline]
pub fn d3d_texture_borrowed_raw<'a>(raw: &'a *mut c_void) -> Option<&'a ID3D11Texture2D> {
    unsafe { ID3D11Texture2D::from_raw_borrowed(raw) }
}

#[inline]
pub fn d3d_device_borrowed_raw<'a>(raw: &'a *mut c_void) -> Option<&'a ID3D11Device> {
    unsafe { ID3D11Device::from_raw_borrowed(raw) }
}

#[inline]
pub fn d3d_context_borrowed_raw<'a>(raw: &'a *mut c_void) -> Option<&'a ID3D11DeviceContext> {
    unsafe { ID3D11DeviceContext::from_raw_borrowed(raw) }
}

pub trait EasyTexture {
    fn get_shared(&self) -> Result<HANDLE>;
    fn desc(&self) -> D3D11_TEXTURE2D_DESC;
    fn size(&self) -> Size;
}

impl EasyTexture for ID3D11Texture2D {
    #[inline]
    fn get_shared(&self) -> Result<HANDLE> {
        Ok(unsafe { self.cast::<IDXGIResource>()?.GetSharedHandle()? })
    }

    #[inline]
    fn size(&self) -> Size {
        let desc = self.desc();
        Size {
            width: desc.Width,
            height: desc.Height,
        }
    }

    #[inline]
    fn desc(&self) -> D3D11_TEXTURE2D_DESC {
        let mut desc = D3D11_TEXTURE2D_DESC::default();
        unsafe {
            self.GetDesc(&mut desc);
        }

        desc
    }
}
