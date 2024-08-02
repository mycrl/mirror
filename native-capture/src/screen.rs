use crate::{CaptureFrameHandler, CaptureHandler, Source, VideoCaptureSourceDescription};

use std::{
    mem::ManuallyDrop,
    ptr::null_mut,
    sync::{atomic::AtomicBool, Arc},
};

use anyhow::{anyhow, Result};
use common::{atomic::EasyAtomic, frame::VideoFrame};
use windows_capture::{
    capture::GraphicsCaptureApiHandler,
    frame::Frame,
    graphics_capture_api::InternalCaptureControl,
    monitor::Monitor,
    settings::{ColorFormat, CursorCaptureSettings, DrawBorderSettings, Settings},
};

use windows::{
    core::Interface,
    Graphics::DirectX::Direct3D11::IDirect3DSurface,
    Win32::Graphics::{
        Direct3D::D3D_DRIVER_TYPE_HARDWARE,
        Direct3D11::{
            D3D11CreateDevice, ID3D11Device, ID3D11DeviceContext, ID3D11Resource, ID3D11Texture2D,
            ID3D11VideoContext, ID3D11VideoDevice, ID3D11VideoProcessor,
            ID3D11VideoProcessorEnumerator, D3D11_BIND_RENDER_TARGET, D3D11_BIND_SHADER_RESOURCE,
            D3D11_CREATE_DEVICE_FLAG, D3D11_SDK_VERSION, D3D11_TEXTURE2D_DESC, D3D11_USAGE_DEFAULT,
            D3D11_VIDEO_FRAME_FORMAT_PROGRESSIVE, D3D11_VIDEO_PROCESSOR_CONTENT_DESC,
            D3D11_VIDEO_PROCESSOR_INPUT_VIEW_DESC, D3D11_VIDEO_PROCESSOR_OUTPUT_VIEW_DESC,
            D3D11_VIDEO_PROCESSOR_STREAM, D3D11_VIDEO_USAGE_PLAYBACK_NORMAL,
            D3D11_VPIV_DIMENSION_TEXTURE2D, D3D11_VPOV_DIMENSION_TEXTURE2D,
        },
        Dxgi::Common::DXGI_FORMAT_NV12,
    },
};

pub struct ScreenCapture {
    is_runing: Arc<AtomicBool>,
}

impl ScreenCapture {
    pub fn new() -> Result<Self> {
        Ok(Self {
            is_runing: Arc::new(AtomicBool::new(false)),
        })
    }
}

impl CaptureHandler for ScreenCapture {
    type Frame = VideoFrame;
    type Error = anyhow::Error;
    type CaptureOptions = VideoCaptureSourceDescription;

    fn get_sources(&self) -> Result<Vec<Source>, Self::Error> {
        let mut sources = Vec::with_capacity(10);
        for it in Monitor::enumerate()? {
            sources.push(Source {
                name: it.name()?,
                id: it.device_name()?,
                index: it.index()? - 1,
            });
        }

        Ok(sources)
    }

    fn start<S: CaptureFrameHandler<Frame = Self::Frame> + 'static>(
        &self,
        opt: Self::CaptureOptions,
        sink: S,
    ) -> Result<(), Self::Error> {
        let source = Monitor::from_index(opt.source.index)?;
        let settings = Settings::new(
            source,
            CursorCaptureSettings::WithoutCursor,
            DrawBorderSettings::Default,
            ColorFormat::Rgba8,
            Context {
                is_runing: self.is_runing.clone(),
                processer: Processer::new(
                    AbsoluteSize {
                        width: source.width()?,
                        height: source.height()?,
                    },
                    AbsoluteSize {
                        width: opt.width,
                        height: opt.height,
                    },
                )?,
            },
        );

        WindowsCapture::start(settings)?;
        Ok(())
    }

    fn stop(&self) -> Result<(), Self::Error> {
        self.is_runing.update(false);
        Ok(())
    }
}

impl Drop for ScreenCapture {
    fn drop(&mut self) {
        let _ = self.stop();
    }
}

struct Context {
    is_runing: Arc<AtomicBool>,
    processer: Processer,
}

struct WindowsCapture {
    ctx: Context,
}

impl GraphicsCaptureApiHandler for WindowsCapture {
    type Flags = Context;
    type Error = anyhow::Error;

    fn new(ctx: Self::Flags) -> Result<Self, Self::Error> {
        Ok(Self { ctx })
    }

    fn on_frame_arrived(
        &mut self,
        frame: &mut Frame,
        capture_control: InternalCaptureControl,
    ) -> Result<(), Self::Error> {
        if !self.ctx.is_runing.get() {
            capture_control.stop();
        }

        self.ctx
            .processer
            .process(unsafe { frame.as_raw_surface() }).unwrap();
        Ok(())
    }
}

#[derive(Debug, Clone, Copy)]
struct AbsoluteSize {
    width: u32,
    height: u32,
}

struct Processer {
    device: ID3D11Device,
    video_device: ID3D11VideoDevice,
    context: ID3D11DeviceContext,
    video_processor: ID3D11VideoProcessor,
    video_processor_enum: ID3D11VideoProcessorEnumerator,
    size: AbsoluteSize,
}

impl Processer {
    fn new(input: AbsoluteSize, output: AbsoluteSize) -> Result<Self> {
        let mut device = None;
        let mut context = None;
        unsafe {
            D3D11CreateDevice(
                None,
                D3D_DRIVER_TYPE_HARDWARE,
                None,
                D3D11_CREATE_DEVICE_FLAG(0),
                None,
                D3D11_SDK_VERSION,
                Some(&mut device),
                None,
                Some(&mut context),
            )?;
        }

        let (device, context) = if let (Some(device), Some(context)) = (device, context) {
            (device, context)
        } else {
            return Err(anyhow!("failed to D3D11CreateDevice"));
        };

        let mut content_desc = D3D11_VIDEO_PROCESSOR_CONTENT_DESC::default();
        content_desc.InputFrameFormat = D3D11_VIDEO_FRAME_FORMAT_PROGRESSIVE;
        content_desc.Usage = D3D11_VIDEO_USAGE_PLAYBACK_NORMAL;
        content_desc.InputWidth = input.width;
        content_desc.InputHeight = input.height;
        content_desc.OutputWidth = output.width;
        content_desc.OutputHeight = output.height;

        let video_device = device.cast::<ID3D11VideoDevice>()?;
        let video_processor_enum =
            unsafe { video_device.CreateVideoProcessorEnumerator(&content_desc)? };
        let video_processor =
            unsafe { video_device.CreateVideoProcessor(&video_processor_enum, 0)? };

        Ok(Self {
            device,
            context,
            video_device,
            video_processor,
            video_processor_enum,
            size: output,
        })
    }

    fn process(&mut self, frame: IDirect3DSurface) -> Result<ID3D11Texture2D> {
        let resource = frame.cast::<ID3D11Resource>()?;

        let mut input_view = None;
        unsafe {
            let mut desc = D3D11_VIDEO_PROCESSOR_INPUT_VIEW_DESC::default();
            desc.ViewDimension = D3D11_VPIV_DIMENSION_TEXTURE2D;
            desc.FourCC = 0;
            self.video_device.CreateVideoProcessorInputView(
                &resource,
                &self.video_processor_enum,
                &desc,
                Some(&mut input_view),
            )?;
        }

        let input_view = if let Some(input_view) = input_view {
            input_view
        } else {
            return Err(anyhow!("failed to create input view"));
        };

        let output_texture = unsafe {
            let mut desc = D3D11_TEXTURE2D_DESC::default();
            desc.Width = self.size.width;
            desc.Height = self.size.height;
            desc.MipLevels = 1;
            desc.ArraySize = 1;
            desc.Format = DXGI_FORMAT_NV12;
            desc.SampleDesc.Count = 1;
            desc.SampleDesc.Quality = 0;
            desc.Usage = D3D11_USAGE_DEFAULT;
            desc.BindFlags = (D3D11_BIND_SHADER_RESOURCE | D3D11_BIND_RENDER_TARGET).0 as u32;
            desc.CPUAccessFlags = 0;
            desc.MiscFlags = 0;

            let mut texture = None;
            self.device
                .CreateTexture2D(&desc, None, Some(&mut texture))?;
            if let Some(texture) = texture {
                texture
            } else {
                return Err(anyhow!("failed to create texture"));
            }
        };

        let mut output_view = None;
        unsafe {
            let mut desc = D3D11_VIDEO_PROCESSOR_OUTPUT_VIEW_DESC::default();
            desc.ViewDimension = D3D11_VPOV_DIMENSION_TEXTURE2D;
            self.video_device.CreateVideoProcessorOutputView(
                &output_texture,
                &self.video_processor_enum,
                &desc,
                Some(&mut output_view),
            )?;
        }

        let output_view = if let Some(output_view) = output_view {
            output_view
        } else {
            return Err(anyhow!("failed to create output view"));
        };

        let context = self.context.cast::<ID3D11VideoContext>()?;
        let mut stream = D3D11_VIDEO_PROCESSOR_STREAM::default();
        stream.Enable = true.into();
        stream.OutputIndex = 0;
        stream.InputFrameOrField = 0;
        stream.PastFrames = 0;
        stream.FutureFrames = 0;
        stream.ppPastSurfaces = null_mut();
        stream.ppFutureSurfaces = null_mut();
        stream.ppPastSurfacesRight = null_mut();
        stream.ppFutureSurfacesRight = null_mut();
        stream.pInputSurface = ManuallyDrop::new(Some(input_view));
        unsafe {
            context.VideoProcessorBlt(&self.video_processor, &output_view, 0, &[stream])?;
        }

        Ok(output_texture)
    }
}
