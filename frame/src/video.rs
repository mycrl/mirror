use std::{mem::ManuallyDrop, ptr::null};

use utils::win32::{IMFValue, MediaFoundationIMFAttributesSetHelper};
use windows::{
    core::{Interface, Result},
    Win32::{
        Graphics::Direct3D11::ID3D11Texture2D,
        Media::MediaFoundation::{
            CLSID_VideoProcessorMFT, IMFMediaBuffer, IMFTransform, MFCreate2DMediaBuffer,
            MFCreateDXGISurfaceBuffer, MFCreateMediaType, MFCreateSample, MFMediaType_Video,
            MFVideoFormat_NV12, MFVideoFormat_RGB32, MFVideoInterlace_Progressive,
            MFT_INPUT_STATUS_ACCEPT_DATA, MFT_MESSAGE_NOTIFY_BEGIN_STREAMING,
            MFT_MESSAGE_NOTIFY_END_OF_STREAM, MFT_OUTPUT_DATA_BUFFER,
            MFT_OUTPUT_STATUS_SAMPLE_READY, MF_E_TRANSFORM_NEED_MORE_INPUT, MF_MT_FRAME_RATE,
            MF_MT_FRAME_SIZE, MF_MT_INTERLACE_MODE, MF_MT_MAJOR_TYPE, MF_MT_SUBTYPE,
        },
        System::Com::{CoCreateInstance, CLSCTX_INPROC_SERVER},
    },
};

/// YCbCr (NV12)
///
/// YCbCr, Y′CbCr, or Y Pb/Cb Pr/Cr, also written as YCBCR or Y′CBCR, is a
/// family of color spaces used as a part of the color image pipeline in video
/// and digital photography systems. Y′ is the luma component and CB and CR are
/// the blue-difference and red-difference chroma components. Y′ (with prime) is
/// distinguished from Y, which is luminance, meaning that light intensity is
/// nonlinearly encoded based on gamma corrected RGB primaries.
///
/// Y′CbCr color spaces are defined by a mathematical coordinate transformation
/// from an associated RGB primaries and white point. If the underlying RGB
/// color space is absolute, the Y′CbCr color space is an absolute color space
/// as well; conversely, if the RGB space is ill-defined, so is Y′CbCr. The
/// transformation is defined in equations 32, 33 in ITU-T H.273. Nevertheless
/// that rule does not apply to P3-D65 primaries used by Netflix with
/// BT.2020-NCL matrix, so that means matrix was not derived from primaries, but
/// now Netflix allows BT.2020 primaries (since 2021). The same happens with
/// JPEG: it has BT.601 matrix derived from System M primaries, yet the
/// primaries of most images are BT.709.
#[repr(C)]
#[derive(Debug)]
pub struct VideoFrame {
    pub width: u32,
    pub height: u32,
    pub data: [*const u8; 2],
    pub linesize: [usize; 2],
}

unsafe impl Sync for VideoFrame {}
unsafe impl Send for VideoFrame {}

impl Default for VideoFrame {
    fn default() -> Self {
        Self {
            width: 0,
            height: 0,
            linesize: [0, 0],
            data: [null(), null()],
        }
    }
}

pub struct VideoSize {
    pub width: u32,
    pub height: u32,
}

pub struct VideoTransform {
    processor: IMFTransform,
    size: VideoSize,
}

unsafe impl Send for VideoTransform {}
unsafe impl Sync for VideoTransform {}

impl VideoTransform {
    #[rustfmt::skip]
    pub fn new(input: VideoSize, input_fps: u8, output: VideoSize, output_fps: u8) -> Result<Self> {
        // Create and configure the Video Processor MFT.
        let processor: IMFTransform =
            unsafe { CoCreateInstance(&CLSID_VideoProcessorMFT, None, CLSCTX_INPROC_SERVER)? };

        // Configure the input type to be a D3D texture in RGB32 format.
        let mut input_ty = unsafe { MFCreateMediaType()? };
        input_ty.set(MF_MT_MAJOR_TYPE, IMFValue::GUID(MFMediaType_Video))?;
        input_ty.set(MF_MT_SUBTYPE, IMFValue::GUID(MFVideoFormat_RGB32))?;
        input_ty.set(MF_MT_INTERLACE_MODE, IMFValue::U32(MFVideoInterlace_Progressive.0 as u32))?;
        input_ty.set(MF_MT_FRAME_SIZE, IMFValue::DoubleU32(input.width, input.height),)?;
        input_ty.set(MF_MT_FRAME_RATE, IMFValue::DoubleU32(input_fps as u32, 1))?;
        unsafe { processor.SetInputType(0, &input_ty, 0)? };

        // Configure the output type to NV12 format.
        let mut output_ty = unsafe { MFCreateMediaType()? };
        output_ty.set(MF_MT_MAJOR_TYPE, IMFValue::GUID(MFMediaType_Video))?;
        output_ty.set(MF_MT_SUBTYPE, IMFValue::GUID(MFVideoFormat_NV12))?;
        output_ty.set(MF_MT_INTERLACE_MODE, IMFValue::U32(MFVideoInterlace_Progressive.0 as u32))?;
        output_ty.set(MF_MT_FRAME_SIZE, IMFValue::DoubleU32(output.width, output.height))?;
        output_ty.set(MF_MT_FRAME_RATE, IMFValue::DoubleU32(output_fps as u32, 1))?;
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
    pub fn process(&self, texture: &ID3D11Texture2D) -> Result<Option<IMFMediaBuffer>> {
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
                        Err(e)
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

impl Drop for VideoTransform {
    fn drop(&mut self) {
        // Notifies a Media Foundation transform (MFT) that an input stream has ended.
        unsafe {
            let _ = self
                .processor
                .ProcessMessage(MFT_MESSAGE_NOTIFY_END_OF_STREAM, 0);
        }
    }
}
