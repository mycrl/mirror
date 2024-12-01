//
// hylarana.h
// hylarana
//
// Created by Panda on 2024/4/1.
//

#ifndef HYLARANA_H
#define HYLARANA_H
#pragma once

#ifndef EXPORT
#ifdef WIN32
#define EXPORT __declspec(dllexport)
#else
#define EXPORT
#endif
#endif

#ifdef WIN32
#include <windows.h>
#endif

#include <stdint.h>
#include <stddef.h>

/**
 * Video frame format.
 */
typedef enum
{
    VIDEO_FORMAT_BGRA,
    VIDEO_FORMAT_RGBA,
    VIDEO_FORMAT_NV12,
    VIDEO_FORMAT_I420,
} HylaranaVideoFormat;

/**
 * Subtype of the video frame.
 */
typedef enum
{
    /**
     * This video frame is from Core video, a type exclusive to the Macos platform.
     */
    VIDEO_SUB_FORMAT_CV_PIXEL_BUF,
    /**
     * Inside this video frame is ID3D11Texture2D.
     */
    VIDEO_SUB_FORMAT_D3D11,
    /**
     * Video frames contain buffers that can be accessed directly through software.
     */
    VIDEO_SUB_FORMAT_SW,
} HylaranaVideoSubFormat;

typedef struct
{
    HylaranaVideoFormat format;
    HylaranaVideoSubFormat sub_format;
    uint32_t width;
    uint32_t height;
    /**
     * If the subformat is SW, the data layout is determined according to the 
     * format and the data corresponds to the plane of the corresponding format, 
     * All other sub formats use data[0].
     */
    void* data[3];
    size_t linesize[3];
} HylaranaVideoFrame;

/**
 * A sample from the audio stream.
 */
typedef struct
{
    int sample_rate;
    /**
     * The number of samples in the current audio frame.
     */
    uint32_t frames;
    /**
     * Pointer to the sample raw buffer.
     */
    int16_t* data;
} HylaranaAudioFrame;

/**
 * Video source type or Audio source type.
 */
typedef enum
{
    /**
     * Camera or video capture card and other devices (and support virtual camera)
     */
    SOURCE_TYPE_CAMERA,
    /**
     * The desktop or monitor corresponds to the desktop in the operating system.
     */
    SOURCE_TYPE_SCREEN,
    /**
     * Audio input and output devices.
     */
    SOURCE_TYPE_AUDIO,
} HylaranaSourceType;

/**
 * Video source or Audio source.
 */
typedef struct
{
    /**
     * Sequence number, which can normally be ignored, in most cases this field 
     * has no real meaning and simply indicates the order in which the device 
     * was acquired internally.
     */
    size_t index;
    HylaranaSourceType type;
    /**
     * Device ID, usually the symbolic link to the device or the address of the 
     * device file handle.
     */
    const char* id;
    const char* name;
    /**
     * Whether or not it is the default device, normally used to indicate 
     * whether or not it is the master device.
     */
    bool is_default;
} HylaranaSource;

typedef struct
{
    HylaranaSource* items;
    size_t capacity;
    size_t size;
} HylaranaSources;

/**
 * Video decoder type.
 */
typedef enum {
    /**
     * see: https://www.openh264.org/
     * 
     * OpenH264 is a codec library which supports H.264 encoding and decoding.
     */
    VIDEO_DECODER_H264,
    /**
     * see: https://learn.microsoft.com/en-us/windows/win32/medfound/direct3d-11-video-apis
     * 
     * Accelerated video decoding using Direct3D 11 Video APIs.
     */
    VIDEO_DECODER_D3D11,
    /**
     * see: https://en.wikipedia.org/wiki/Intel_Quick_Sync_Video
     * 
     * Intel Quick Sync Video is Intel’s brand for its dedicated video encoding 
     * and decoding hardware core.
     */
    VIDEO_DECODER_QSV,
    /**
     * see: https://developer.apple.com/documentation/videotoolbox
     * 
     * VideoToolbox is a low-level framework that provides direct access to 
     * hardware encoders and decoders.
     */
    VIDEO_DECODER_VIDEOTOOLBOX,
} HylaranaVideoDecoderType;

/**
 * Video encoder type.
 */
typedef enum 
{
    /**
     * see: https://www.videolan.org/developers/x264.html
     * 
     * x264 is a free software library and application for encoding video 
     * streams into the H.264/MPEG-4 AVC compression format, and is released 
     * under the terms of the GNU GPL.
     */
    VIDEO_ENCODER_X264,
    /**
     * see: https://en.wikipedia.org/wiki/Intel_Quick_Sync_Video
     * 
     * Intel Quick Sync Video is Intel’s brand for its dedicated video encoding 
     * and decoding hardware core.
     */
    VIDEO_ENCODER_QSV,
    /**
     * see: https://developer.apple.com/documentation/videotoolbox
     * 
     * VideoToolbox is a low-level framework that provides direct access to 
     * hardware encoders and decoders.
     */
    VIDEO_ENCODER_VIDEOTOOLBOX,
} HylaranaVideoEncoderType;

/**
 * Back-end implementation of graphics.
 */
typedef enum 
{
    /**
     * Backend implemented using D3D11, which is supported on an older device 
     * and platform and has better performance performance and memory footprint, 
     * but only on windows.
     */
    RENDER_BACKEND_DIRECT3D_11,
    /**
     * Cross-platform graphics backends implemented using WebGPUs are supported 
     * on a number of common platforms or devices.
     */
    RENDER_BACKEND_WEBGPU,
} HylaranaVideoRenderBackend;

/**
 * Transport layer strategies.
 */
typedef enum
{
    /**
     * In straight-through mode, the sender creates an SRT server and the receiver 
     * connects directly to the sender via the SRT protocol.
     * 
     * For the sender, the network address is the address to which the SRT server 
     * binds and listens.
     * 
     * example: 0.0.0.0:8080
     * 
     * For the receiving end, the network address is the address of the SRT server 
     * on the sending end.
     * 
     * example: 192.168.1.100:8080
     */
    STRATEGY_DIRECT,
    /**
     * Forwarding mode, where the sender and receiver pass data through a relay 
     * server.
     * 
     * The network address is the address of the transit server.
     */
    STRATEGY_RELAY,
    /**
     * UDP multicast mode, where the sender sends multicast packets into the 
     * current network and the receiver processes the multicast packets.
     * 
     * The sender and receiver use the same address, which is a combination of 
     * multicast address + port.
     * 
     * example: 239.0.0.1:8080
     */
    STRATEGY_MULTICAST,
} HylaranaTransportStrategy;

/**
 * Transport configuration.
 */
typedef struct
{
    HylaranaTransportStrategy strategy;
    /**
     * hylarana address.
     */
    const char* address;
    /**
     * The size of the maximum transmission unit of the network, which is
     * related to the settings of network devices such as routers or switches,
     * the recommended value is 1400.
     */
    size_t mtu;
} HylaranaTransportOptions;

typedef struct
{
    /**
     * Video encoder settings, possible values are `h264_qsv`, `h264_nvenc`,
     * `libx264` and so on.
     */
    HylaranaVideoEncoderType codec;
    /**
     * Frame rate setting in seconds.
     */
    uint8_t frame_rate;
    /**
     * The width of the video.
     */
    uint32_t width;
    /**
     * The height of the video.
     */
    uint32_t height;
    /**
     * The bit rate of the video encoding.
     */
    uint64_t bit_rate;
    /**
     * Keyframe Interval, used to specify how many frames apart to output a
     * keyframe.
     */
    uint32_t key_frame_interval;
} HylaranaVideoEncoderOptions;

/**
 * Description of the audio encoding.
 */
typedef struct
{
    /**
     * The sample rate of the audio, in seconds.
     */
    uint64_t sample_rate;
    /**
     * The bit rate of the video encoding.
     */
    uint64_t bit_rate;
} HylaranaAudioEncoderOptions;

typedef struct
{
    HylaranaSource* source;
    HylaranaVideoEncoderOptions options;
} HylaranaVideoTrackOptions;

typedef struct
{
    HylaranaSource* source;
    HylaranaAudioEncoderOptions options;
} HylaranaAudioTrackOptions;

/**
 * Options of the media stream.
 */
typedef struct
{
    HylaranaVideoTrackOptions* video;
    HylaranaAudioTrackOptions* audio;
} HylaranaSenderMediaOptions;

/**
 * Sender configuration.
 */
typedef struct
{
    HylaranaSenderMediaOptions media;
    HylaranaTransportOptions transport;
} HylaranaSenderOptions;

/**
 * Receiver media codec configuration.
 */
typedef struct
{
    HylaranaVideoDecoderType video;
} HylaranaReceiverCodecOptions;

/**
 * Receiver configuration.
 */
typedef struct
{
    HylaranaReceiverCodecOptions codec;
    HylaranaTransportOptions transport;
} HylaranaReceiverOptions;

/**
 * A raw window handle for Win32.
 * 
 * This variant is used on Windows systems.
 */
typedef struct
{
    /**
     * A Win32 HWND handle.
     */
    void* hwnd;
    uint32_t width;
    uint32_t height;
} HylaranaWin32Window;

/**
 * A raw window handle for Xlib.
 * 
 * This variant is likely to show up anywhere someone manages to get X11
 * working that Xlib can be built for, which is to say, most (but not all) Unix
 * systems.
 */
typedef struct
{
    /**
     * An Xlib Window.
     */
    unsigned long window;
    /**
     * A pointer to an Xlib Display.
     */
    void* display;
    /**
     * An X11 screen to use with this display handle.
     */
    int screen;
    uint32_t width;
    uint32_t height;
} HylaranaXlibWindow;

/**
 * A raw window handle for Wayland.
 * 
 * This variant should be expected anywhere Wayland works, which is currently
 * some subset of unix systems.
 */
typedef struct
{
    /**
     * A pointer to a wl_surface.
     */
    void* surface;
    /**
     * A pointer to a wl_display.
     */
    void* display;
    uint32_t width;
    uint32_t height;
} HylaranaWaylandWindow;

/**
 * A raw window handle for AppKit.
 * 
 * This variant is likely to be used on macOS, although Mac Catalyst
 * $arch-apple-ios-macabi targets.
 */
typedef struct
{
    /**
     * A pointer to an NSView object.
     */
    void* window;
    uint32_t width;
    uint32_t height;
} HylaranaAppkitWindow;

typedef union
{
    HylaranaWin32Window win32;
    HylaranaXlibWindow xlib;
    HylaranaWaylandWindow wayland;
    HylaranaAppkitWindow appkit;
} HylaranaWindowValue;

typedef enum
{
    WINDOW_TYPE_WIN32,
    WINDOW_TYPE_XLIB,
    WINDOW_TYPE_WAYLAND,
    WINDOW_TYPE_APPKIT,
} HylaranaWindowType;

/**
 * A window handle for a particular windowing system.
 */
typedef struct
{    
    HylaranaWindowType type;
    HylaranaWindowValue value;
} HylaranaWindowOptions;

/**
 * Video render configure.
 */
typedef struct
{    
    HylaranaWindowOptions window;
    HylaranaVideoRenderBackend backend;
} HylaranaVideoRenderOptions;

typedef enum
{
    /**
     * Both audio and video will play.
     */
    AV_PLAYER_ALL,
    /**
     * Play video only.
     */
    AV_PLAYER_ONLY_VIDEO,
    /**
     * Play audio only.
     */
    AV_PLAYER_ONLY_AUDIO,
    /**
     * Nothing plays.
     */
    AV_PLAYER_QUIET,
} HylaranaAVFrameStreamPlayerType;

typedef union
{
    HylaranaVideoRenderOptions some;
    struct {} none;
} HylaranaAVFrameStreamPlayerValue;

/**
 * Configuration of the audio and video streaming player.
 */
typedef struct
{
   HylaranaAVFrameStreamPlayerType type;
   HylaranaAVFrameStreamPlayerValue value;
} HylaranaAVFrameStreamPlayerOptions;

/**
 * Creates the configuration of the player and the callback function is the 
 * callback when the stream is closed.
 */
typedef struct
{
    HylaranaAVFrameStreamPlayerOptions options;
    void (*close)(void* ctx);
    void* ctx;
} HylaranaPlayerOptions;

typedef struct
{
    /**
     * Callback occurs when the video frame is updated. The video frame format
     * is fixed to NV12. Be careful not to call blocking methods inside the
     * callback, which will seriously slow down the encoding and decoding
     * pipeline.
     *
     * YCbCr (NV12)
     *
     * YCbCr, Y′CbCr, or Y Pb/Cb Pr/Cr, also written as YCBCR or Y′CBCR, is a
     * family of color spaces used as a part of the color image pipeline in
     * video and digital photography systems. Y′ is the luma component and
     * CB and CR are the blue-difference and red-difference chroma
     * components. Y′ (with prime) is distinguished from Y, which is
     * luminance, meaning that light intensity is nonlinearly encoded based
     * on gamma corrected RGB primaries.
     *
     * Y′CbCr color spaces are defined by a mathematical coordinate
     * transformation from an associated RGB primaries and white point. If
     * the underlying RGB color space is absolute, the Y′CbCr color space
     * is an absolute color space as well; conversely, if the RGB space is
     * ill-defined, so is Y′CbCr. The transformation is defined in
     * equations 32, 33 in ITU-T H.273. Nevertheless that rule does not
     * apply to P3-D65 primaries used by Netflix with BT.2020-NCL matrix,
     * so that means matrix was not derived from primaries, but now Netflix
     * allows BT.2020 primaries (since 2021). The same happens with
     * JPEG: it has BT.601 matrix derived from System M primaries, yet the
     * primaries of most images are BT.709.
     */
    bool (*video)(void* ctx, HylaranaVideoFrame* frame);
    /**
     * Callback is called when the audio frame is updated. The audio frame
     * format is fixed to PCM. Be careful not to call blocking methods inside
     * the callback, which will seriously slow down the encoding and decoding
     * pipeline.
     *
     * Pulse-code modulation
     *
     * Pulse-code modulation (PCM) is a method used to digitally represent
     * analog signals. It is the standard form of digital audio in
     * computers, compact discs, digital telephony and other digital audio
     * applications. In a PCM stream, the amplitude of the analog signal is
     * sampled at uniform intervals, and each sample is quantized to the
     * nearest value within a range of digital steps.
     *
     * Linear pulse-code modulation (LPCM) is a specific type of PCM in which
     * the quantization levels are linearly uniform. This is in contrast to
     * PCM encodings in which quantization levels vary as a function of
     * amplitude (as with the A-law algorithm or the μ-law algorithm).
     * Though PCM is a more general term, it is often used to describe data
     * encoded as LPCM.
     *
     * A PCM stream has two basic properties that determine the stream's
     * fidelity to the original analog signal: the sampling rate, which is
     * the number of times per second that samples are taken; and the bit
     * depth, which determines the number of possible digital values that
     * can be used to represent each sample.
     */
    bool (*audio)(void* ctx, HylaranaAudioFrame* frame);
    /**
     * Callback when the stream is closed. This may be because the external
     * side actively calls the close, or the audio and video packets cannot be
     * sent (the network is disconnected), etc.
     */
    void (*close)(void* ctx);
    void* ctx;
} HylaranaFrameSink;

/**
 * Because Linux does not have DllMain, you need to call it manually to achieve
 * similar behavior.
 */
#ifndef WIN32

/**
 * Initialize the environment, which must be initialized before using the SDK.
 */
EXPORT bool hylarana_startup();

/**
 * Cleans up the environment when the SDK exits, and is recommended to be
 * called when the application exits.
 */
EXPORT void hylarana_shutdown();

#endif // !WIN32

/**
 * Get capture sources.
 */
EXPORT HylaranaSources hylarana_get_sources(HylaranaSourceType kind);

/**
 * Because `Sources` are allocated internally, they also need to be released
 * internally.
 */
EXPORT void hylarana_sources_destroy(HylaranaSources* sources);

typedef const void* HylaranaSender;

/**
 * Create a sender, specify a bound NIC address, you can pass callback to
 * get the device screen or sound callback, callback can be null, if it is
 * null then it means no callback data is needed.
 */
EXPORT HylaranaSender hylarana_create_sender(HylaranaSenderOptions options, HylaranaFrameSink sink, char* id);

/**
 * Destroy sender.
 */
EXPORT void hylarana_sender_destroy(HylaranaSender sender);

/**
 * Create the sender. the difference is that this function creates the player together, 
 * you don't need to implement the stream sink manually, the player manages it automatically.
 */
EXPORT HylaranaSender hylarana_create_sender_with_player(HylaranaSenderOptions options, HylaranaPlayerOptions player, char* id);

/**
 * Destroy sender with player.
 */
EXPORT void hylarana_sender_with_player_destroy(HylaranaSender sender);

typedef const void* HylaranaReceiver;

/**
 * Create a receiver, specify a bound NIC address, you can pass callback to
 * get the sender's screen or sound callback, callback can not be null.
 */
EXPORT HylaranaReceiver hylarana_create_receiver(const char* id, HylaranaReceiverOptions options, HylaranaFrameSink sink);

/**
 * Destroy receiver.
 */
EXPORT void hylarana_receiver_destroy(HylaranaReceiver receiver);

/**
 * Create the receiver. the difference is that this function creates the player together, 
 * you don't need to implement the stream sink manually, the player manages it automatically.
 */
EXPORT HylaranaReceiver hylarana_create_receiver_with_player(const char* id, HylaranaReceiverOptions options, HylaranaPlayerOptions player);

/**
 * Destroy receiver with player.
 */
EXPORT void hylarana_receiver_with_player_destroy(HylaranaReceiver receiver);

typedef const void* HylaranaProperties;

/**
 * Create a properties.
 */
EXPORT HylaranaProperties hylarana_create_properties();

/**
 * Adds key pair values to the property list, which is Map inside.
 */
EXPORT bool hylarana_properties_insert(HylaranaProperties properties, const char* key, const char* value);

/**
 * Get value from the property list, which is Map inside.
 */
EXPORT bool hylarana_properties_get(HylaranaProperties properties, const char* key, char* value);

/**
 * Destroy the properties.
 */
EXPORT void hylarana_properties_destroy(HylaranaProperties properties);

typedef const void* HylaranaDiscovery;

/**
 * Register the service, the service type is fixed, you can customize the
 * port number, id is the identifying information of the service, used to
 * distinguish between different publishers, in properties you can add
 * customized data to the published service.
 */
EXPORT HylaranaDiscovery hylarana_discovery_register(uint16_t port, HylaranaProperties properties);

typedef void (*HylaranaDiscoveryQueryCallback)(void* ctx, const char** addrs, size_t addrs_size, HylaranaProperties properties);

/**
 * Query the registered service, the service type is fixed, when the query
 * is published the callback function will call back all the network
 * addresses of the service publisher as well as the attribute information.
 */
EXPORT HylaranaDiscovery hylarana_discovery_query(HylaranaDiscoveryQueryCallback callback, void* ctx);

/**
 * Destroy the discovery.
 */
EXPORT void hylarana_discovery_destroy(HylaranaDiscovery discovery);

#endif // HYLARANA_H
