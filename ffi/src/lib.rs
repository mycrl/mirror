#[cfg(any(target_os = "windows", target_os = "linux"))]
pub mod desktop {
    use std::{
        ffi::{c_char, c_int},
        fmt::Debug,
        io::stdout,
        ptr::null_mut,
        sync::atomic::AtomicBool,
    };

    #[cfg(any(target_os = "windows", target_os = "linux"))]
    use std::{
        ffi::{c_void, CString},
        mem::ManuallyDrop,
    };

    use chrono::Local;
    use fern::{
        colors::{Color, ColoredLevelConfig},
        Dispatch as LogDispatch,
    };

    use frame::{AudioFrame, VideoFrame};
    use log::LevelFilter;
    use mirror::Window;
    use utils::{atomic::EasyAtomic, strings::Strings};

    #[cfg(any(target_os = "windows", target_os = "linux"))]
    use capture::{Capture, SourceType};

    #[cfg(target_os = "windows")]
    use utils::win32::windows::core::Interface;

    // In fact, this is a package that is convenient for recording errors. If the
    // result is an error message, it is output to the log. This function does not
    // make any changes to the result.
    #[inline]
    fn checker<T, E: Debug>(result: Result<T, E>) -> Result<T, E> {
        if let Err(e) = &result {
            log::error!("{:?}", e);

            if cfg!(debug_assertions) {
                println!("{:#?}", e);
            }
        }

        result
    }

    /// Windows yes! The Windows dynamic library has an entry, so just
    /// initialize the logger and set the process priority at the entry.
    #[no_mangle]
    #[allow(non_snake_case)]
    #[cfg(target_os = "windows")]
    extern "system" fn DllMain(
        _module: u32,
        call_reason: usize,
        reserved: *const std::ffi::c_void,
    ) -> bool {
        match call_reason {
            1 /* DLL_PROCESS_ATTACH */ => mirror_startup(),
            0 /* DLL_PROCESS_DETACH */ => {
                if reserved.is_null() {
                    mirror_shutdown();
                }

                true
            },
            _ => true,
        }
    }

    /// Initialize the environment, which must be initialized before using the
    /// SDK.
    #[no_mangle]
    pub extern "C" fn mirror_startup() -> bool {
        let func = || {
            let mut logger = LogDispatch::new()
                .level(LevelFilter::Info)
                .level_for("wgpu", LevelFilter::Warn)
                .level_for("wgpu_core", LevelFilter::Warn)
                .level_for("wgpu_hal", LevelFilter::Warn)
                .level_for("wgpu_hal::auxil::dxgi::exception", LevelFilter::Error);

            if cfg!(debug_assertions) {
                let colors = ColoredLevelConfig::new()
                    .info(Color::Blue)
                    .warn(Color::Yellow)
                    .error(Color::Red);

                logger = logger
                    .format(move |out, message, record| {
                        out.finish(format_args!(
                            "[{}] - ({}) - {}",
                            colors.color(record.level()),
                            record.file_static().unwrap_or("*"),
                            message
                        ))
                    })
                    .chain(stdout());
            } else {
                logger = logger.format(move |out, message, record| {
                    out.finish(format_args!(
                        "{} - [{}] - ({}) - {}",
                        Local::now().format("%m-%d %H:%M:%S"),
                        record.level(),
                        record.file_static().unwrap_or("*"),
                        message
                    ))
                });

                #[cfg(target_os = "windows")]
                {
                    if std::fs::metadata("./logs").is_err() {
                        std::fs::create_dir("./logs")?;
                    }

                    logger = logger.chain(fern::DateBased::new("logs/", "%Y-%m-%d-mirror.log"));
                }

                #[cfg(target_os = "linux")]
                {
                    logger = logger.chain(
                        syslog::unix(syslog::Formatter3164 {
                            facility: syslog::Facility::LOG_USER,
                            process: "mirror".to_owned(),
                            hostname: None,
                            pid: 0,
                        })
                        .map_err(|e| anyhow::anyhow!("{:?}", e))?,
                    );
                }
            }

            logger.apply()?;
            mirror::startup()?;
            Ok::<_, anyhow::Error>(())
        };

        checker(func()).is_ok()
    }

    /// Cleans up the environment when the SDK exits, and is recommended to be
    /// called when the application exits.
    #[no_mangle]
    pub extern "C" fn mirror_shutdown() {
        log::info!("extern api: mirror quit");

        let _ = checker(mirror::shutdown());
    }

    #[repr(C)]
    #[derive(Debug, Clone, Copy)]
    pub struct MirrorDescriptor {
        pub server: *const c_char,
        pub multicast: *const c_char,
        pub mtu: usize,
    }

    impl TryInto<transport::TransportDescriptor> for MirrorDescriptor {
        type Error = anyhow::Error;

        fn try_into(self) -> Result<transport::TransportDescriptor, Self::Error> {
            Ok(transport::TransportDescriptor {
                multicast: Strings::from(self.multicast).to_string()?.parse()?,
                server: Strings::from(self.server).to_string()?.parse()?,
                mtu: self.mtu,
            })
        }
    }

    #[repr(C)]
    pub struct Mirror(mirror::Mirror);

    /// Create mirror.
    #[no_mangle]
    pub extern "C" fn mirror_create(options: MirrorDescriptor) -> *const Mirror {
        log::info!("extern api: mirror create");

        let func = || mirror::Mirror::new(options.try_into()?);
        checker(func())
            .map(|mirror| Box::into_raw(Box::new(Mirror(mirror))))
            .unwrap_or_else(|_| null_mut()) as *const _
    }

    /// Release mirror.
    #[no_mangle]
    pub extern "C" fn mirror_destroy(mirror: *const Mirror) {
        assert!(!mirror.is_null());

        log::info!("extern api: mirror destroy");
        drop(unsafe { Box::from_raw(mirror as *mut Mirror) });
    }

    /// Get direct3d device.
    #[no_mangle]
    #[cfg(target_os = "windows")]
    pub extern "C" fn mirror_get_direct3d_device(mirror: *const Mirror) -> *mut c_void {
        assert!(!mirror.is_null());

        unsafe { &*mirror }
            .0
            .get_direct3d_device()
            .map(|it| it.device.as_raw())
            .unwrap_or_else(|| null_mut())
    }

    /// Get direct3d device context.
    #[no_mangle]
    #[cfg(target_os = "windows")]
    pub extern "C" fn mirror_get_direct3d_device_context(mirror: *const Mirror) -> *mut c_void {
        assert!(!mirror.is_null());

        unsafe { &*mirror }
            .0
            .get_direct3d_device()
            .map(|it| it.context.as_raw())
            .unwrap_or_else(|| null_mut())
    }

    #[repr(C)]
    #[derive(Debug)]
    #[cfg(any(target_os = "windows", target_os = "linux"))]
    pub struct Source {
        index: usize,
        kind: SourceType,
        id: *const c_char,
        name: *const c_char,
        is_default: bool,
    }

    #[cfg(any(target_os = "windows", target_os = "linux"))]
    impl TryInto<capture::Source> for &Source {
        type Error = anyhow::Error;

        fn try_into(self) -> Result<capture::Source, Self::Error> {
            Ok(capture::Source {
                name: Strings::from(self.name).to_string()?,
                id: Strings::from(self.id).to_string()?,
                is_default: self.is_default,
                index: self.index,
                kind: self.kind,
            })
        }
    }

    #[repr(C)]
    #[derive(Debug)]
    #[cfg(any(target_os = "windows", target_os = "linux"))]
    pub struct Sources {
        items: *mut Source,
        capacity: usize,
        size: usize,
    }

    /// Get capture sources from sender.
    #[no_mangle]
    #[cfg(any(target_os = "windows", target_os = "linux"))]
    pub extern "C" fn mirror_get_sources(kind: SourceType) -> Sources {
        log::info!("extern api: mirror get sources: kind={:?}", kind);

        let mut items = ManuallyDrop::new(
            Capture::get_sources(kind.into())
                .unwrap_or_else(|_| Vec::new())
                .into_iter()
                .map(|item| {
                    log::info!("source: {:?}", item);

                    Source {
                        index: item.index,
                        is_default: item.is_default,
                        kind: SourceType::from(item.kind),
                        id: CString::new(item.id).unwrap().into_raw(),
                        name: CString::new(item.name).unwrap().into_raw(),
                    }
                })
                .collect::<Vec<Source>>(),
        );

        Sources {
            items: items.as_mut_ptr(),
            capacity: items.capacity(),
            size: items.len(),
        }
    }

    /// Because `Sources` are allocated internally, they also need to be
    /// released internally.
    #[no_mangle]
    #[cfg(any(target_os = "windows", target_os = "linux"))]
    pub extern "C" fn mirror_sources_destroy(sources: *const Sources) {
        assert!(!sources.is_null());

        let sources = unsafe { &*sources };
        for item in unsafe { Vec::from_raw_parts(sources.items, sources.size, sources.capacity) } {
            drop(unsafe { CString::from_raw(item.id as *mut _) });
            drop(unsafe { CString::from_raw(item.name as *mut _) });
        }
    }

    #[repr(C)]
    #[derive(Clone, Copy)]
    pub struct FrameSink {
        /// Callback occurs when the video frame is updated. The video frame
        /// format is fixed to NV12. Be careful not to call blocking
        /// methods inside the callback, which will seriously slow down
        /// the encoding and decoding pipeline.
        ///
        /// YCbCr (NV12)
        ///
        /// YCbCr, Y′CbCr, or Y Pb/Cb Pr/Cr, also written as YCBCR or Y′CBCR, is
        /// a family of color spaces used as a part of the color image
        /// pipeline in video and digital photography systems. Y′ is the
        /// luma component and CB and CR are the blue-difference and
        /// red-difference chroma components. Y′ (with prime) is
        /// distinguished from Y, which is luminance, meaning that light
        /// intensity is nonlinearly encoded based on gamma corrected
        /// RGB primaries.
        ///
        /// Y′CbCr color spaces are defined by a mathematical coordinate
        /// transformation from an associated RGB primaries and white point. If
        /// the underlying RGB color space is absolute, the Y′CbCr color space
        /// is an absolute color space as well; conversely, if the RGB space is
        /// ill-defined, so is Y′CbCr. The transformation is defined in
        /// equations 32, 33 in ITU-T H.273. Nevertheless that rule does not
        /// apply to P3-D65 primaries used by Netflix with BT.2020-NCL matrix,
        /// so that means matrix was not derived from primaries, but now Netflix
        /// allows BT.2020 primaries (since 2021). The same happens with
        /// JPEG: it has BT.601 matrix derived from System M primaries, yet the
        /// primaries of most images are BT.709.
        pub video: Option<extern "C" fn(ctx: usize, frame: *const VideoFrame) -> bool>,
        /// Callback is called when the audio frame is updated. The audio frame
        /// format is fixed to PCM. Be careful not to call blocking methods
        /// inside the callback, which will seriously slow down the
        /// encoding and decoding pipeline.
        ///
        /// Pulse-code modulation
        ///
        /// Pulse-code modulation (PCM) is a method used to digitally represent
        /// analog signals. It is the standard form of digital audio in
        /// computers, compact discs, digital telephony and other digital audio
        /// applications. In a PCM stream, the amplitude of the analog signal is
        /// sampled at uniform intervals, and each sample is quantized to the
        /// nearest value within a range of digital steps.
        ///
        /// Linear pulse-code modulation (LPCM) is a specific type of PCM in
        /// which the quantization levels are linearly uniform. This is
        /// in contrast to PCM encodings in which quantization levels
        /// vary as a function of amplitude (as with the A-law algorithm
        /// or the μ-law algorithm). Though PCM is a more general term,
        /// it is often used to describe data encoded as LPCM.
        ///
        /// A PCM stream has two basic properties that determine the stream's
        /// fidelity to the original analog signal: the sampling rate, which is
        /// the number of times per second that samples are taken; and the bit
        /// depth, which determines the number of possible digital values that
        /// can be used to represent each sample.
        pub audio: Option<extern "C" fn(ctx: usize, frame: *const AudioFrame) -> bool>,
        /// Callback when the sender is closed. This may be because the external
        /// side actively calls the close, or the audio and video packets cannot
        /// be sent (the network is disconnected), etc.
        pub close: Option<extern "C" fn(ctx: usize)>,
        pub ctx: usize,
    }

    impl Into<mirror::FrameSink> for FrameSink {
        fn into(self) -> mirror::FrameSink {
            // Record whether it is closed
            let is_closed = AtomicBool::new(false);

            mirror::FrameSink {
                video: Box::new(move |frame: &VideoFrame| {
                    if let Some(callback) = &self.video {
                        callback(self.ctx, frame)
                    } else {
                        true
                    }
                }),
                audio: Box::new(move |frame: &AudioFrame| {
                    if let Some(callback) = &self.audio {
                        callback(self.ctx, frame)
                    } else {
                        true
                    }
                }),
                close: Box::new(move || {
                    // I thought about it carefully. The closing hand should only trigger the
                    // callback once. There are too many places in the system that will trigger the
                    // closing callback. It is not easy to manage the status between components.
                    // Here, the closing status is directly recorded. If it has been closed, it will
                    // not be processed anymore.
                    if !is_closed.get() {
                        is_closed.update(true);

                        if let Some(callback) = &self.close {
                            callback(self.ctx);

                            log::info!("extern api: call close callback");
                        }
                    }
                }),
            }
        }
    }

    #[repr(C)]
    #[derive(Debug, Clone, Copy)]
    pub enum VideoEncoderType {
        X264,
        Qsv,
        Cuda,
    }

    impl Into<codec::VideoEncoderType> for VideoEncoderType {
        fn into(self) -> codec::VideoEncoderType {
            match self {
                Self::X264 => codec::VideoEncoderType::X264,
                Self::Qsv => codec::VideoEncoderType::Qsv,
                Self::Cuda => codec::VideoEncoderType::Cuda,
            }
        }
    }

    /// Video Codec Configuretion.
    #[repr(C)]
    #[derive(Debug, Clone, Copy)]
    pub struct VideoDescriptor {
        pub codec: VideoEncoderType,
        pub frame_rate: u8,
        pub width: u32,
        pub height: u32,
        pub bit_rate: u64,
        pub key_frame_interval: u32,
    }

    #[cfg(any(target_os = "windows", target_os = "linux"))]
    impl TryInto<mirror::VideoDescriptor> for VideoDescriptor {
        type Error = anyhow::Error;

        fn try_into(self) -> Result<mirror::VideoDescriptor, Self::Error> {
            Ok(mirror::VideoDescriptor {
                codec: self.codec.into(),
                key_frame_interval: self.key_frame_interval,
                frame_rate: self.frame_rate,
                width: self.width,
                height: self.height,
                bit_rate: self.bit_rate,
            })
        }
    }

    /// Audio Codec Configuration.
    #[repr(C)]
    #[derive(Debug, Clone, Copy)]
    pub struct AudioDescriptor {
        pub sample_rate: u64,
        pub bit_rate: u64,
    }

    #[cfg(any(target_os = "windows", target_os = "linux"))]
    impl Into<mirror::AudioDescriptor> for AudioDescriptor {
        fn into(self) -> mirror::AudioDescriptor {
            mirror::AudioDescriptor {
                sample_rate: self.sample_rate,
                bit_rate: self.bit_rate,
            }
        }
    }

    #[repr(C)]
    #[derive(Debug)]
    #[cfg(any(target_os = "windows", target_os = "linux"))]
    pub struct SenderSourceDescriptor<T> {
        source: *const Source,
        options: T,
    }

    #[repr(C)]
    #[derive(Debug)]
    #[cfg(any(target_os = "windows", target_os = "linux"))]
    pub struct SenderDescriptor {
        video: *const SenderSourceDescriptor<VideoDescriptor>,
        audio: *const SenderSourceDescriptor<AudioDescriptor>,
        multicast: bool,
    }

    #[cfg(any(target_os = "windows", target_os = "linux"))]
    impl TryInto<mirror::SenderDescriptor> for SenderDescriptor {
        type Error = anyhow::Error;

        // Both video and audio are optional, so the type conversion here is a bit more
        // complicated.
        #[rustfmt::skip]
        fn try_into(self) -> Result<mirror::SenderDescriptor, Self::Error> {
            let mut options = mirror::SenderDescriptor {
                multicast: self.multicast,
                audio: None,
                video: None,
            };

            if !self.video.is_null() {
                let video = unsafe { &*self.video };
                let settings: mirror::VideoDescriptor = video.options.try_into()?;

                // Check whether the external parameters are configured correctly to 
                // avoid some clowns inserting some inexplicable parameters.
                anyhow::ensure!(settings.width % 4 == 0 && settings.width <= 4096, "invalid video width");
                anyhow::ensure!(settings.height % 4 == 0 && settings.height <= 2560, "invalid video height");
                anyhow::ensure!(settings.frame_rate <= 60, "invalid video frame rate");

                options.video = Some((
                    unsafe { &*video.source }.try_into()?,
                    settings,
                ));
            }

            if !self.audio.is_null() {
                let audio = unsafe { &*self.audio };
                options.audio = Some((
                    unsafe { &*audio.source }.try_into()?,
                    audio.options.try_into()?,
                ));
            }

            Ok(options)
        }
    }

    #[repr(C)]
    #[cfg(any(target_os = "windows", target_os = "linux"))]
    pub struct Sender(mirror::Sender);

    /// Create a sender, specify a bound NIC address, you can pass callback to
    /// get the device screen or sound callback, callback can be null, if it is
    /// null then it means no callback data is needed.
    #[no_mangle]
    #[rustfmt::skip]
    #[cfg(any(target_os = "windows", target_os = "linux"))]
    pub extern "C" fn mirror_create_sender(
        mirror: *const Mirror,
        id: c_int,
        options: SenderDescriptor,
        sink: FrameSink,
    ) -> *const Sender {
        assert!(!mirror.is_null());
    
        log::info!("extern api: mirror create sender");
    
        let func = || {
            let options: mirror::SenderDescriptor = options.try_into()?;
            log::info!("mirror create options={:?}", options);
            
            unsafe { &*mirror }
                .0
                .create_sender(id as u32, options, sink.into())
        };
    
        checker(func())
        .map(|sender| Box::into_raw(Box::new(Sender(sender))))
        .unwrap_or_else(|_| null_mut())
    }

    /// Set whether the sender uses multicast transmission.
    #[no_mangle]
    #[cfg(any(target_os = "windows", target_os = "linux"))]
    pub extern "C" fn mirror_sender_set_multicast(sender: *const Sender, is_multicast: bool) {
        assert!(!sender.is_null());

        log::info!("extern api: mirror set sender multicast={}", is_multicast);
        unsafe { &*sender }.0.set_multicast(is_multicast);
    }

    /// Get whether the sender uses multicast transmission.
    #[no_mangle]
    #[cfg(any(target_os = "windows", target_os = "linux"))]
    pub extern "C" fn mirror_sender_get_multicast(sender: *const Sender) -> bool {
        assert!(!sender.is_null());

        log::info!("extern api: mirror get sender multicast");
        unsafe { &*sender }.0.get_multicast()
    }

    /// Close sender.
    #[no_mangle]
    #[cfg(any(target_os = "windows", target_os = "linux"))]
    pub extern "C" fn mirror_sender_destroy(sender: *const Sender) {
        assert!(!sender.is_null());

        log::info!("extern api: mirror close sender");
        drop(unsafe { Box::from_raw(sender as *mut Sender) })
    }

    #[repr(C)]
    pub struct Receiver(mirror::Receiver);

    #[repr(C)]
    #[derive(Debug, Clone, Copy)]
    pub enum VideoDecoderType {
        D3D11,
        Qsv,
        Cuda,
    }

    impl Into<codec::VideoDecoderType> for VideoDecoderType {
        fn into(self) -> codec::VideoDecoderType {
            match self {
                Self::D3D11 => codec::VideoDecoderType::D3D11,
                Self::Qsv => codec::VideoDecoderType::Qsv,
                Self::Cuda => codec::VideoDecoderType::Cuda,
            }
        }
    }

    /// Create a receiver, specify a bound NIC address, you can pass callback to
    /// get the sender's screen or sound callback, callback can not be null.
    #[no_mangle]
    pub extern "C" fn mirror_create_receiver(
        mirror: *const Mirror,
        id: c_int,
        codec: VideoDecoderType,
        sink: FrameSink,
    ) -> *const Receiver {
        assert!(!mirror.is_null());

        log::info!("extern api: mirror create receiver");

        let func = || {
            unsafe { &*mirror }.0.create_receiver(
                id as u32,
                mirror::ReceiverDescriptor {
                    video: codec.into(),
                },
                sink.into(),
            )
        };

        checker(func())
            .map(|receiver| Box::into_raw(Box::new(Receiver(receiver))))
            .unwrap_or_else(|_| null_mut())
    }

    /// Close receiver.
    #[no_mangle]
    pub extern "C" fn mirror_receiver_destroy(receiver: *const Receiver) {
        assert!(!receiver.is_null());

        log::info!("extern api: mirror close receiver");
        drop(unsafe { Box::from_raw(receiver as *mut Receiver) })
    }

    #[repr(C)]
    struct RawRenderer(mirror::Render);

    /// Creating a window renderer.
    #[no_mangle]
    #[allow(unused_variables)]
    extern "C" fn renderer_create(hwnd: *mut c_void) -> *mut RawRenderer {
        let func =
            || Ok::<RawRenderer, anyhow::Error>(RawRenderer(mirror::Render::new(Window(hwnd))?));

        checker(func())
            .map(|ret| Box::into_raw(Box::new(ret)))
            .unwrap_or_else(|_| null_mut())
    }

    /// Push the video frame into the renderer, which will update the window
    /// texture.
    #[no_mangle]
    extern "C" fn renderer_on_video(render: *mut RawRenderer, frame: *const VideoFrame) -> bool {
        assert!(!render.is_null() && !frame.is_null());

        checker(checker(
            unsafe { &mut *render }.0.on_video(unsafe { &*frame }),
        ))
        .is_ok()
    }

    /// Push the audio frame into the renderer, which will append to audio
    /// queue.
    #[no_mangle]
    extern "C" fn renderer_on_audio(render: *mut RawRenderer, frame: *const AudioFrame) -> bool {
        assert!(!render.is_null() && !frame.is_null());

        checker(checker(
            unsafe { &mut *render }.0.on_audio(unsafe { &*frame }),
        ))
        .is_ok()
    }

    /// Destroy the window renderer.
    #[no_mangle]
    extern "C" fn renderer_destroy(render: *mut RawRenderer) {
        assert!(!render.is_null());

        let _ = unsafe { Box::from_raw(render) };
    }
}

#[cfg(target_os = "android")]
pub mod android {
    mod adapter;
    mod common;
    mod logger;

    use std::{ffi::c_void, ptr::null_mut, sync::Arc, thread};

    use adapter::AndroidStreamReceiverAdapter;
    use common::{catcher, copy_from_byte_array, JVM};
    use jni::{
        objects::{JByteArray, JClass, JObject, JString},
        sys::JNI_VERSION_1_6,
        JNIEnv, JavaVM,
    };

    use logger::AndroidLogger;
    use transport::{
        adapter::{StreamReceiverAdapter, StreamReceiverAdapterExt, StreamSenderAdapter},
        Transport, TransportDescriptor,
    };

    /// JNI_OnLoad
    ///
    /// jint JNI_OnLoad(JavaVM *vm, void *reserved);
    ///
    /// The VM calls JNI_OnLoad when the native library is loaded (for example,
    /// through System.loadLibrary). JNI_OnLoad must return the JNI version
    /// needed by the native library.
    /// In order to use any of the new JNI functions, a native library must
    /// export a JNI_OnLoad function that returns JNI_VERSION_1_2. If the
    /// native library does not export a JNI_OnLoad function, the VM assumes
    /// that the library only requires JNI version JNI_VERSION_1_1. If the
    /// VM does not recognize the version number returned by JNI_OnLoad, the
    /// VM will unload the library and act as if the library was +never
    /// loaded.
    ///
    /// JNI_Onload_L(JavaVM *vm, void *reserved);
    ///
    /// If a library L is statically linked, then upon the first invocation of
    /// System.loadLibrary("L") or equivalent API, a JNI_OnLoad_L function will
    /// be invoked with the same arguments and expected return value as
    /// specified for the JNI_OnLoad function. JNI_OnLoad_L must return the
    /// JNI version needed by the native library. This version must be
    /// JNI_VERSION_1_8 or later. If the VM does not recognize the version
    /// number returned by JNI_OnLoad_L, the VM will act as if the library
    /// was never loaded.
    ///
    /// LINKAGE:
    /// Exported from native libraries that contain native method
    /// implementation.
    #[no_mangle]
    #[allow(non_snake_case)]
    pub extern "system" fn JNI_OnLoad(vm: JavaVM, _: *mut c_void) -> i32 {
        AndroidLogger::init();
        transport::startup();
        JVM.lock().unwrap().replace(vm);

        JNI_VERSION_1_6
    }

    /// JNI_OnUnload
    ///
    /// void JNI_OnUnload(JavaVM *vm, void *reserved);
    ///
    /// The VM calls JNI_OnUnload when the class loader containing the native
    /// library is garbage collected. This function can be used to perform
    /// cleanup operations. Because this function is called in an unknown
    /// context (such as from a finalizer), the programmer should be
    /// conservative on using Java VM services, and refrain from arbitrary
    /// Java call-backs. Note that JNI_OnLoad and JNI_OnUnload are two
    /// functions optionally supplied by JNI libraries, not exported from
    /// the VM.
    ///
    /// JNI_OnUnload_L(JavaVM *vm, void *reserved);
    ///
    /// When the class loader containing a statically linked native library L is
    /// garbage collected, the VM will invoke the JNI_OnUnload_L function of the
    /// library if such a function is exported. This function can be used to
    /// perform cleanup operations. Because this function is called in an
    /// unknown context (such as from a finalizer), the programmer should be
    /// conservative on using Java VM services, and refrain from arbitrary
    /// Java call-backs.
    ///
    /// Informational Note:
    /// The act of loading a native library is the complete process of making
    /// the library and its native entry points known and registered to the
    /// Java VM and runtime. Note that simply performing operating system
    /// level operations to load a native library, such as dlopen on a
    /// UNIX(R) system, does not fully accomplish this goal. A native
    /// function is normally called from the Java class loader to perform a
    /// call to the host operating system that will load the library into
    /// memory and return a handle to the native library. This handle will
    /// be stored and used in subsequent searches for native library
    /// entry points. The Java native class loader will complete the load
    /// process once the handle is successfully returned to register the
    /// library.
    ///
    /// LINKAGE:
    /// Exported from native libraries that contain native method
    /// implementation.
    #[no_mangle]
    #[allow(non_snake_case)]
    pub extern "system" fn JNI_OnUnload(_: JavaVM, _: *mut c_void) {
        transport::shutdown();
    }

    mod objects {
        use anyhow::{anyhow, Ok};
        use jni::{
            objects::{JObject, JValueGen},
            JNIEnv,
        };

        use transport::adapter::{StreamBufferInfo, StreamKind};

        /// /**
        ///  * Streaming data information.
        ///  */
        /// data class StreamBufferInfo(val kind: Int) {
        ///     var flags: Int = 0;
        /// }
        pub fn to_stream_buffer_info(
            env: &mut JNIEnv,
            info: &JObject,
        ) -> anyhow::Result<StreamBufferInfo> {
            let kind = if let JValueGen::Int(kind) = env.get_field(info, "kind", "I")? {
                kind
            } else {
                return Err(anyhow!("kind not a int."));
            };

            let flags = if let JValueGen::Int(flags) = env.get_field(info, "flags", "I")? {
                flags
            } else {
                return Err(anyhow!("flags not a int."));
            };

            let timestamp =
                if let JValueGen::Long(timestamp) = env.get_field(info, "timestamp", "J")? {
                    timestamp as u64
                } else {
                    return Err(anyhow!("timestamp not a long."));
                };

            Ok(
                match StreamKind::try_from(kind as u8).map_err(|_| anyhow!("kind unreachable"))? {
                    StreamKind::Video => StreamBufferInfo::Video(flags, timestamp),
                    StreamKind::Audio => StreamBufferInfo::Audio(flags, timestamp),
                },
            )
        }
    }

    /// /**
    ///  * Create a stream receiver adapter where the return value is a
    ///  * pointer to the instance, and you need to check that the returned
    ///  * pointer is not Null.
    ///  */
    /// private external fun createStreamReceiverAdapter(adapter:
    /// ReceiverAdapter): Long
    #[no_mangle]
    #[allow(non_snake_case)]
    pub extern "system" fn Java_com_github_mycrl_mirror_Mirror_createStreamReceiverAdapter(
        mut env: JNIEnv,
        _this: JClass,
        callback: JObject,
    ) -> *const Arc<StreamReceiverAdapter> {
        catcher(&mut env, |env| {
            let adapter = AndroidStreamReceiverAdapter {
                callback: env.new_global_ref(callback)?,
            };

            let stream_adapter = StreamReceiverAdapter::new();
            let stream_adapter_ = Arc::downgrade(&stream_adapter);
            thread::Builder::new()
                .name("MirrorJniStreamReceiverThread".to_string())
                .spawn(move || {
                    while let Some(stream_adapter) = stream_adapter_.upgrade() {
                        if let Some((buf, kind, flags, timestamp)) = stream_adapter.next() {
                            if !adapter.sink(buf, kind, flags, timestamp) {
                                break;
                            }
                        } else {
                            break;
                        }
                    }

                    log::info!("StreamReceiverAdapter is closed");

                    adapter.close();
                })?;

            Ok(Box::into_raw(Box::new(stream_adapter)))
        })
        .unwrap_or_else(null_mut)
    }

    /// /**
    ///  * Free the stream receiver adapter instance pointer.
    ///  */
    /// private external fun releaseStreamReceiverAdapter(adapter: Long)
    #[no_mangle]
    #[allow(non_snake_case)]
    pub extern "system" fn Java_com_github_mycrl_mirror_Mirror_releaseStreamReceiverAdapter(
        _env: JNIEnv,
        _this: JClass,
        ptr: *const Arc<StreamReceiverAdapter>,
    ) {
        unsafe { Box::from_raw(ptr as *mut Arc<StreamReceiverAdapter>) }.close();
    }

    /// /**
    ///  * Creates a mirror instance, the return value is a pointer, and you
    ///    need to
    ///  * check that the pointer is valid.
    ///  */
    /// private external fun createMirror(
    ///     bind: String,
    ///     adapterFactory: Long
    /// ): Long
    #[no_mangle]
    #[allow(non_snake_case)]
    pub extern "system" fn Java_com_github_mycrl_mirror_Mirror_createMirror(
        mut env: JNIEnv,
        _this: JClass,
        server: JString,
        multicast: JString,
        mtu: i32,
    ) -> *const Transport {
        catcher(&mut env, |env| {
            let server: String = env.get_string(&server)?.into();
            let multicast: String = env.get_string(&multicast)?.into();

            Ok(Box::into_raw(Box::new(Transport::new(
                TransportDescriptor {
                    server: server.parse()?,
                    multicast: multicast.parse()?,
                    mtu: mtu as usize,
                },
            )?)))
        })
        .unwrap_or_else(null_mut)
    }

    /// /**
    ///  * Free the mirror instance pointer.
    ///  */
    /// private external fun releaseMirror(mirror: Long)
    #[no_mangle]
    #[allow(non_snake_case)]
    pub extern "system" fn Java_com_github_mycrl_mirror_Mirror_releaseMirror(
        _env: JNIEnv,
        _this: JClass,
        ptr: *const Transport,
    ) {
        drop(unsafe { Box::from_raw(ptr as *mut Transport) })
    }

    /// /**
    ///  * Creates an instance of the stream sender adapter, the return value is
    ///    a
    ///  * pointer and you need to check if the pointer is valid.
    ///  */
    /// private external fun createStreamSenderAdapter(kind: Int): Long
    #[no_mangle]
    #[allow(non_snake_case)]
    pub extern "system" fn Java_com_github_mycrl_mirror_Mirror_createStreamSenderAdapter(
        _env: JNIEnv,
        _this: JClass,
    ) -> *const Arc<StreamSenderAdapter> {
        Box::into_raw(Box::new(StreamSenderAdapter::new(false)))
    }

    /// /**
    ///  * Get whether the sender uses multicast transmission
    ///  */
    /// private external fun senderGetMulticast(adapter: Long): Boolean
    #[no_mangle]
    #[allow(non_snake_case)]
    pub extern "system" fn Java_com_github_mycrl_mirror_Mirror_senderGetMulticast(
        _env: JNIEnv,
        _this: JClass,
        ptr: *const Arc<StreamSenderAdapter>,
    ) -> i32 {
        unsafe { &*ptr }.get_multicast() as i32
    }

    /// /**
    ///  * Set whether the sender uses multicast transmission
    ///  */
    /// private external fun senderSetMulticast(adapter: Long, is_multicast:
    /// Boolean)
    #[no_mangle]
    #[allow(non_snake_case)]
    pub extern "system" fn Java_com_github_mycrl_mirror_Mirror_senderSetMulticast(
        _env: JNIEnv,
        _this: JClass,
        ptr: *const Arc<StreamSenderAdapter>,
        is_multicast: i32,
    ) {
        unsafe { &*ptr }.set_multicast(is_multicast != 0)
    }

    /// /**
    ///  * Release the stream sender adapter.
    ///  */
    /// private external fun releaseStreamSenderAdapter(adapter: Long)
    #[no_mangle]
    #[allow(non_snake_case)]
    pub extern "system" fn Java_com_github_mycrl_mirror_Mirror_releaseStreamSenderAdapter(
        _env: JNIEnv,
        _this: JClass,
        ptr: *const Arc<StreamSenderAdapter>,
    ) {
        unsafe { Box::from_raw(ptr as *mut Arc<StreamSenderAdapter>) }.close();
    }

    /// /**
    ///  * Creates the sender, the return value indicates whether the creation
    ///  * was successful or not.
    ///  */
    /// private external fun createSender(
    ///     mirror: Long,
    ///     id: Int,
    ///     description: ByteArray,
    ///     adapter: Long
    /// ): Boolean
    #[no_mangle]
    #[allow(non_snake_case)]
    pub extern "system" fn Java_com_github_mycrl_mirror_Mirror_createSender(
        mut env: JNIEnv,
        _this: JClass,
        mirror: *const Transport,
        id: i32,
        adapter: *const Arc<StreamSenderAdapter>,
    ) -> i32 {
        catcher(&mut env, |_| {
            unsafe { &*mirror }.create_sender(id as u32, unsafe { &*adapter })?;
            Ok(true)
        })
        .unwrap_or(false) as i32
    }

    /// /**
    ///  * Sends the packet to the sender instance.
    ///  */
    /// private external fun sendBufToSender(
    ///     adapter: Long,
    ///     buf: ByteArray,
    ///     info: BufferInfo
    /// )
    #[no_mangle]
    #[allow(non_snake_case)]
    pub extern "system" fn Java_com_github_mycrl_mirror_Mirror_sendBufToSender(
        mut env: JNIEnv,
        _this: JClass,
        adapter: *const Arc<StreamSenderAdapter>,
        info: JObject,
        buf: JByteArray,
    ) {
        catcher(&mut env, |env| {
            let buf = copy_from_byte_array(env, &buf)?;
            let info = objects::to_stream_buffer_info(env, &info)?;
            unsafe { &*adapter }.send(buf, info);

            Ok(())
        });
    }

    /// /**
    ///  * Creates the receiver, the return value indicates whether the creation
    ///  * was successful or not.
    ///  */
    /// private external fun createReceiver(
    ///     mirror: Long,
    ///     addr: String,
    ///     adapter: Long
    /// ): Boolean
    #[no_mangle]
    #[allow(non_snake_case)]
    pub extern "system" fn Java_com_github_mycrl_mirror_Mirror_createReceiver(
        mut env: JNIEnv,
        _this: JClass,
        mirror: *const Transport,
        id: i32,
        adapter: *const Arc<StreamReceiverAdapter>,
    ) -> i32 {
        catcher(&mut env, |_| {
            unsafe { &*mirror }.create_receiver(id as u32, unsafe { &*adapter })?;
            Ok(true)
        })
        .unwrap_or(false) as i32
    }
}
