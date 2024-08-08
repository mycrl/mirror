use super::{IMFValue, MediaFoundationIMFAttributesSetHelper};
use crate::{
    CaptureHandler, FrameArrived, Size, Source, SourceType, VideoCaptureSourceDescription,
};

use std::{
    mem::ManuallyDrop,
    ptr::null_mut,
    sync::{atomic::AtomicBool, Arc, Mutex, RwLock},
    thread,
    time::Duration,
};

use anyhow::{anyhow, Result};
use frame::VideoFrame;
use utils::atomic::EasyAtomic;
use windows::{
    core::Interface,
    Win32::{
        Graphics::Direct3D11::ID3D11Texture2D,
        Media::MediaFoundation::{
            CLSID_VideoProcessorMFT, IMF2DBuffer, IMFMediaBuffer, IMFTransform,
            MFCreate2DMediaBuffer, MFCreateDXGISurfaceBuffer, MFCreateMediaType, MFCreateSample,
            MFMediaType_Video, MFVideoFormat_NV12, MFVideoFormat_RGB32,
            MFVideoInterlace_Progressive, MFT_INPUT_STATUS_ACCEPT_DATA,
            MFT_MESSAGE_NOTIFY_BEGIN_STREAMING, MFT_MESSAGE_NOTIFY_END_OF_STREAM,
            MFT_OUTPUT_DATA_BUFFER, MFT_OUTPUT_STATUS_SAMPLE_READY, MF_E_TRANSFORM_NEED_MORE_INPUT,
            MF_MT_FRAME_RATE, MF_MT_FRAME_SIZE, MF_MT_INTERLACE_MODE, MF_MT_MAJOR_TYPE,
            MF_MT_SUBTYPE,
        },
        System::Com::{CoCreateInstance, CLSCTX_INPROC_SERVER},
    },
};

use windows_capture::{
    capture::{CaptureControl, GraphicsCaptureApiHandler},
    frame::Frame,
    graphics_capture_api::InternalCaptureControl,
    monitor::Monitor,
    settings::{ColorFormat, CursorCaptureSettings, DrawBorderSettings, Settings},
};

struct Transform {
    processor: IMFTransform,
    size: Size,
}

unsafe impl Send for Transform {}
unsafe impl Sync for Transform {}

impl Transform {
    #[rustfmt::skip]
    fn new(input: Size, output: Size, fps: u8) -> Result<Self> {
        // Create and configure the Video Processor MFT.
        let processor: IMFTransform =
            unsafe { CoCreateInstance(&CLSID_VideoProcessorMFT, None, CLSCTX_INPROC_SERVER)? };

        // Configure the input type to be a D3D texture in RGB32 format.
        let mut input_ty = unsafe { MFCreateMediaType()? };
        input_ty.set(MF_MT_MAJOR_TYPE, IMFValue::GUID(MFMediaType_Video))?;
        input_ty.set(MF_MT_SUBTYPE, IMFValue::GUID(MFVideoFormat_RGB32))?;
        input_ty.set(MF_MT_INTERLACE_MODE, IMFValue::U32(MFVideoInterlace_Progressive.0 as u32))?;
        input_ty.set(MF_MT_FRAME_SIZE, IMFValue::DoubleU32(input.width, input.height),)?;
        input_ty.set(MF_MT_FRAME_RATE, IMFValue::DoubleU32(fps as u32, 1))?;
        unsafe { processor.SetInputType(0, &input_ty, 0)? };

        // Configure the output type to NV12 format.
        let mut output_ty = unsafe { MFCreateMediaType()? };
        output_ty.set(MF_MT_MAJOR_TYPE, IMFValue::GUID(MFMediaType_Video))?;
        output_ty.set(MF_MT_SUBTYPE, IMFValue::GUID(MFVideoFormat_NV12))?;
        output_ty.set(MF_MT_INTERLACE_MODE, IMFValue::U32(MFVideoInterlace_Progressive.0 as u32))?;
        output_ty.set(MF_MT_FRAME_SIZE, IMFValue::DoubleU32(output.width, output.height))?;
        output_ty.set(MF_MT_FRAME_RATE, IMFValue::DoubleU32(fps as u32, 1))?;
        unsafe { processor.SetOutputType(0, &output_ty, 0)? };

        // Call IMFTransform::ProcessMessage with the MFT_MESSAGE_NOTIFY_BEGIN_STREAMING
        // message. This message requests the MFT to allocate any resources it needs
        // during streaming.
        unsafe { processor.ProcessMessage(MFT_MESSAGE_NOTIFY_BEGIN_STREAMING, 0)? };

        Ok(Self {
            processor,
            size: output,
        })
    }

    // Process Data
    // An MFT is designed to be a reliable state machine. It does not make any calls
    // back to the client.
    //
    // 1. Call IMFTransform::ProcessMessage with the
    //    MFT_MESSAGE_NOTIFY_BEGIN_STREAMING message. This message requests the MFT
    //    to allocate any resources it needs during streaming.
    //
    // 2. Call IMFTransform::ProcessInput on at least one input stream to deliver an
    //    input sample to the MFT.
    //
    // 3. (Optional.) Call IMFTransform::GetOutputStatus to query whether the MFT
    //    can generate an output sample. If the method returns S_OK, check the
    //    pdwFlags parameter. If pdwFlags contains the
    //    MFT_OUTPUT_STATUS_SAMPLE_READY flag, go to step 4. If pdwFlags is zero, go
    //    back to step 2. If the method returns E_NOTIMPL, go to step 4.
    //
    // 4. Call IMFTransform::ProcessOutput to get output data.
    //  * If the method returns MF_E_TRANSFORM_NEED_MORE_INPUT, it means the MFT
    //    requires more input data; go back to step 2.
    //  * If the method returns MF_E_TRANSFORM_STREAM_CHANGE, it means the number of
    //    output streams has changed, or the output format has changed. The client
    //    might need to query for new stream // identifiers or set new media types.
    //    For more information, see the documentation for ProcessOutput.
    //
    // 5. If there is still input data to process, go to step 2. If the MFT has
    //    consumed all of the available input data, proceed to step 6.
    //
    // 6. Call ProcessMessage with the MFT_MESSAGE_NOTIFY_END_OF_STREAM message.
    //
    // 7. Call ProcessMessage with the MFT_MESSAGE_COMMAND_DRAIN message.
    //
    // 8. Call ProcessOutput to get the remaining output. Repeat this step until the
    //    method returns MF_E_TRANSFORM_NEED_MORE_INPUT. This return value signals
    //    that all of the output has been drained from the MFT. (Do not treat this
    //    as an error condition.)
    //
    // The sequence described here keeps as little data as possible in the MFT.
    // After every call to ProcessInput, the client attempts to get output. Several
    // input samples might be needed to produce one output sample, or a single input
    // sample might generate several output samples. The optimal behavior for the
    // client is to pull output samples from the MFT until the MFT requires more
    // input.
    //
    // However, the MFT should be able to handle a different order of method calls
    // by the client. For example, the client might simply alternate between calls
    // to ProcessInput and ProcessOutput. The MFT should restrict the amount of
    // input that it gets by returning MF_E_NOTACCEPTING from ProcessInput whenever
    // it has some output to produce.
    //
    // The order of method calls described here is not the only valid sequence of
    // events. For example, steps 3 and 4 assume that the client starts with the
    // input types and then tries the output types. The client can also reverse this
    // order and start with the output types. In either case, if the MFT requires
    // the opposite order, it should return the error code
    // MF_E_TRANSFORM_TYPE_NOT_SET.
    //
    // The client can call informational methods, such as GetInputCurrentType and
    // GetOutputStreamInfo, at any time during streaming. The client can also
    // attempt to change the media types at any time. The MFT should return an error
    // code if this is not a valid operation. In short, MFTs should assume very
    // little about the order of operations, other than what is documented in the
    // calls themselves.
    fn process(&self, texture: &ID3D11Texture2D) -> Result<Option<IMFMediaBuffer>> {
        if unsafe { self.processor.GetInputStatus(0)? } == MFT_INPUT_STATUS_ACCEPT_DATA.0 as u32 {
            // Creates a media buffer to manage a Microsoft DirectX Graphics Infrastructure
            // (DXGI) surface.
            let buffer =
                unsafe { MFCreateDXGISurfaceBuffer(&ID3D11Texture2D::IID, texture, 0, false)? };

            // Call IMFTransform::ProcessInput on at least one input stream to deliver an
            // input sample to the MFT.
            let sample = unsafe { MFCreateSample()? };
            unsafe { sample.AddBuffer(&buffer)? };
            unsafe { self.processor.ProcessInput(0, &sample, 0)? };
        }

        // Call IMFTransform::GetOutputStatus to query whether the MFT can generate an
        // output sample.
        Ok(
            if unsafe { self.processor.GetOutputStatus()? }
                == MFT_OUTPUT_STATUS_SAMPLE_READY.0 as u32
            {
                let buffer = unsafe {
                    MFCreate2DMediaBuffer(
                        self.size.width,
                        self.size.height,
                        MFVideoFormat_NV12.data1,
                        false,
                    )?
                };

                let sample = unsafe { MFCreateSample()? };
                unsafe { sample.AddBuffer(&buffer)? };

                let mut status = 0;
                let mut buffers = [MFT_OUTPUT_DATA_BUFFER::default()];

                // Generates output from the current input data.
                buffers[0].dwStreamID = 0;
                buffers[0].pSample = ManuallyDrop::new(Some(sample));
                if let Err(e) =
                    unsafe { self.processor.ProcessOutput(0, &mut buffers, &mut status) }
                {
                    return if e.code() != MF_E_TRANSFORM_NEED_MORE_INPUT {
                        Err(anyhow!("{:?}", e))
                    } else {
                        Ok(None)
                    };
                }

                unsafe { ManuallyDrop::drop(&mut buffers[0].pSample) };
                Some(buffer)
            } else {
                None
            },
        )
    }
}

impl Drop for Transform {
    fn drop(&mut self) {
        log::info!("windows screen capture transform is drop");

        // Notifies a Media Foundation transform (MFT) that an input stream has ended.
        unsafe {
            let _ = self
                .processor
                .ProcessMessage(MFT_MESSAGE_NOTIFY_END_OF_STREAM, 0);
        }
    }
}

struct WindowsCapture {
    texture: Arc<RwLock<Option<ID3D11Texture2D>>>,
    status: Arc<AtomicBool>,
}

impl GraphicsCaptureApiHandler for WindowsCapture {
    type Flags = (Box<dyn FrameArrived<Frame = VideoFrame>>, Context);
    type Error = anyhow::Error;

    fn new((mut arrived, ctx): Self::Flags) -> Result<Self, Self::Error> {
        let texture: Arc<RwLock<Option<ID3D11Texture2D>>> = Default::default();
        let status: Arc<AtomicBool> = Arc::new(AtomicBool::new(true));

        let mut frame = VideoFrame::default();
        frame.width = ctx.options.size.width;
        frame.height = ctx.options.size.height;

        let texture_ = Arc::downgrade(&texture);
        let status_ = status.clone();
        thread::Builder::new()
            .name("WindowsScreenCaptureTransformThread".to_string())
            .spawn(move || {
                let mut func = || {
                    while let Some(texture) = texture_.upgrade() {
                        if let Some(texture) = texture.read().unwrap().as_ref() {
                            if let Some(buffer) = ctx.transform.process(texture)? {
                                // If the buffer contains 2-D image data (such as an uncompressed
                                // video frame), you should query
                                // the buffer for the IMF2DBuffer
                                // interface. The methods on
                                // IMF2DBuffer are optimized for 2-D data.
                                let texture = buffer.cast::<IMF2DBuffer>()?;

                                // Gives the caller access to the memory in the buffer.
                                let mut stride = 0;
                                let mut data = null_mut();
                                unsafe { texture.Lock2D(&mut data, &mut stride)? };

                                frame.data[0] = data;
                                frame.data[1] =
                                    unsafe { data.add(stride as usize * frame.height as usize) };
                                frame.linesize = [stride as usize, stride as usize];
                                if !arrived.sink(&frame) {
                                    break;
                                }

                                // Unlocks a buffer that was previously locked.
                                unsafe { texture.Unlock2D()? };
                            }
                        }

                        thread::sleep(Duration::from_millis(1000 / ctx.options.fps as u64));
                    }

                    Ok::<(), anyhow::Error>(())
                };

                if let Err(e) = func() {
                    log::error!("WindowsScreenCaptureTransformThread error={:?}", e);
                } else {
                    log::info!("WindowsScreenCaptureTransformThread is closed");
                }

                status_.update(false);
            })?;

        Ok(Self { texture, status })
    }

    fn on_frame_arrived(
        &mut self,
        frame: &mut Frame,
        capture_control: InternalCaptureControl,
    ) -> Result<(), Self::Error> {
        if self.status.get() {
            // Video conversion always runs at a fixed frame rate. Here we simply update the
            // latest frame to effectively solve the frame rate mismatch problem.
            if let Ok(mut texture) = self.texture.write() {
                drop(texture.replace(frame.texture()?));
            }
        } else {
            log::info!("windows screen capture control stop");
            capture_control.stop();
        }

        Ok(())
    }

    fn on_closed(&mut self) -> Result<(), Self::Error> {
        self.status.update(false);
        Ok(())
    }
}

struct Context {
    transform: Transform,
    options: VideoCaptureSourceDescription,
}

#[derive(Default)]
pub struct ScreenCapture(Mutex<Option<CaptureControl<WindowsCapture, anyhow::Error>>>);

impl CaptureHandler for ScreenCapture {
    type Frame = VideoFrame;
    type Error = anyhow::Error;
    type CaptureOptions = VideoCaptureSourceDescription;

    fn get_sources() -> Result<Vec<Source>, Self::Error> {
        let primary_name = Monitor::primary()?.name()?;

        let mut displays = Vec::with_capacity(10);
        for item in Monitor::enumerate()? {
            displays.push(Source {
                name: item.name()?,
                index: item.index()?,
                id: item.device_name()?,
                kind: SourceType::Screen,
                is_default: item.name()? == primary_name,
            });
        }

        Ok(displays)
    }

    fn start<S: FrameArrived<Frame = Self::Frame> + 'static>(
        &self,
        options: Self::CaptureOptions,
        arrived: S,
    ) -> Result<(), Self::Error> {
        let source = Monitor::enumerate()?
            .into_iter()
            .find(|it| it.name().ok() == Some(options.source.name.clone()))
            .ok_or_else(|| anyhow!("not found the source"))?;

        // Start capturing the screen. This runs in a free thread. If it runs in the
        // current thread, you will encounter problems with Winrt runtime
        // initialization.
        if let Some(control) = self
            .0
            .lock()
            .unwrap()
            .replace(WindowsCapture::start_free_threaded(Settings {
                cursor_capture: CursorCaptureSettings::WithoutCursor,
                draw_border: DrawBorderSettings::Default,
                color_format: ColorFormat::Bgra8,
                item: source,
                flags: (
                    Box::new(arrived),
                    Context {
                        transform: Transform::new(
                            Size {
                                width: source.width()?,
                                height: source.height()?,
                            },
                            options.size,
                            options.fps,
                        )?,
                        options,
                    },
                ),
            })?)
        {
            control.stop()?;
        }

        Ok(())
    }

    fn stop(&self) -> Result<(), Self::Error> {
        if let Some(control) = self.0.lock().unwrap().take() {
            control.stop()?;
        }

        Ok(())
    }
}
