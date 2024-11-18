//
// hylarana.h
// hylarana
//
// Created by Panda on 2024/4/1.
//

#ifndef MIRROR_H
#define MIRROR_H
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

typedef enum
{
    VIDEO_FORMAT_BGRA,
    VIDEO_FORMAT_RGBA,
    VIDEO_FORMAT_NV12,
    VIDEO_FORMAT_I420,
} HylaranaVideoFormat;

typedef enum
{
    VIDEO_SUB_FORMAT_D3D11,
    VIDEO_SUB_FORMAT_SW,
} HylaranaVideoSubFormat;

typedef struct
{
    HylaranaVideoFormat format;
    HylaranaVideoSubFormat sub_format;
    uint32_t width;
    uint32_t height;
    void* data[3];
    size_t linesize[3];
} HylaranaVideoFrame;

typedef struct
{
    int sample_rate;
    uint32_t frames;
    int16_t* data;
} HylaranaAudioFrame;

typedef enum
{
    SOURCE_TYPE_CAMERA,
    SOURCE_TYPE_SCREEN,
    SOURCE_TYPE_AUDIO,
} HylaranaSourceType;

typedef struct
{
    size_t index;
    HylaranaSourceType type;
    const char* id;
    const char* name;
    bool is_default;
} HylaranaSource;

typedef struct
{
    HylaranaSource* items;
    size_t capacity;
    size_t size;
} HylaranaSources;

typedef enum {
    VIDEO_DECODER_H264,
    VIDEO_DECODER_D3D11,
    VIDEO_DECODER_QSV,
    VIDEO_DECODER_CUDA,
    VIDEO_DECODER_VIDEOTOOLBOX,
} HylaranaVideoDecoderType;

typedef enum 
{
    VIDEO_ENCODER_X264,
    VIDEO_ENCODER_QSV,
    VIDEO_ENCODER_CUDA,
    VIDEO_ENCODER_VIDEOTOOLBOX,
} HylaranaVideoEncoderType;

typedef enum 
{
    RENDER_BACKEND_DX11,
    RENDER_BACKEND_WGPU,
} HylaranaGraphicsBackend;

typedef enum
{
    STRATEGY_DIRECT,
    STRATEGY_RELAY,
    STRATEGY_MULTICAST,
} HylaranaStrategy;

typedef struct
{
    HylaranaStrategy strategy;
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
} HylaranaDescriptor;

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
} HylaranaVideoEncoderDescriptor;

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
} HylaranaAudioEncoderDescriptor;

typedef struct
{
    HylaranaSource* source;
    HylaranaVideoEncoderDescriptor encoder;
} HylaranaVideoDescriptor;

typedef struct
{
    HylaranaSource* source;
    HylaranaAudioEncoderDescriptor encoder;
} HylaranaAudioDescriptor;

typedef struct
{
    HylaranaVideoDescriptor* video;
    HylaranaAudioDescriptor* audio;
    HylaranaDescriptor transport;
} HylaranaSenderDescriptor;

typedef struct
{
    HylaranaVideoDecoderType video;
    HylaranaDescriptor transport;
} HylaranaReceiverDescriptor;


typedef const void* HylaranaSender;
typedef const void* HylaranaReceiver;

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
     * Callback when the sender is closed. This may be because the external
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

/**
 * Create a sender, specify a bound NIC address, you can pass callback to
 * get the device screen or sound callback, callback can be null, if it is
 * null then it means no callback data is needed.
 */
EXPORT HylaranaSender hylarana_create_sender(char* id, HylaranaSenderDescriptor options, HylaranaFrameSink sink);

/**
 * Close sender.
 */
EXPORT void hylarana_sender_destroy(HylaranaSender sender);

/**
 * Create a receiver, specify a bound NIC address, you can pass callback to
 * get the sender's screen or sound callback, callback can not be null.
 */
EXPORT HylaranaReceiver hylarana_create_receiver(const char* id, HylaranaReceiverDescriptor options, HylaranaFrameSink sink);

/**
 * Close receiver.
 */
EXPORT void hylarana_receiver_destroy(HylaranaReceiver receiver);

typedef const void* HylaranaWindowHandle;
typedef const void* HylaranaRender;

#ifdef WIN32

/**
 * Raw window handle for Win32.
 * 
 * This variant is used on Windows systems.
 */
EXPORT HylaranaWindowHandle hylarana_create_window_handle_for_win32(HWND hwnd, uint32_t width, uint32_t height);

#endif // WIN32

#ifdef LINUX

/**
 * A raw window handle for Xlib.
 *
 * This variant is likely to show up anywhere someone manages to get X11
 * working that Xlib can be built for, which is to say, most (but not all)
 * Unix systems.
 */
EXPORT HylaranaWindowHandle hylarana_create_window_handle_for_xlib(uint32_t hwnd, void* display, int screen, uint32_t width, uint32_t height);

/**
 * A raw window handle for Xcb.
 *
 * This variant is likely to show up anywhere someone manages to get X11
 * working that XCB can be built for, which is to say, most (but not all)
 * Unix systems.
 */
EXPORT HylaranaWindowHandle hylarana_create_window_handle_for_xcb(uint32_t hwnd, void* display, int screen, uint32_t width, uint32_t height);

/**
 * A raw window handle for Wayland.
 *
 * This variant should be expected anywhere Wayland works, which is
 * currently some subset of unix systems.
 */
EXPORT HylaranaWindowHandle hylarana_create_window_handle_for_wayland(void* hwnd, void* display, uint32_t width, uint32_t height);

#endif

#ifdef MACOS

/**
 * A raw window handle for AppKit.
 *
 * This variant is likely to be used on macOS, although Mac Catalyst 
 * ($arch-apple-ios-macabi targets, which can notably use UIKit or AppKit) can 
 * also use it despite being target_os = "ios".
 */
EXPORT HylaranaWindowHandle hylarana_create_window_handle_for_appkit(void* view, uint32_t width, uint32_t height);

#endif

/**
 * Destroy the window handle.
 */
EXPORT void hylarana_window_handle_destroy(HylaranaWindowHandle hwnd);

/**
 * Creating a window renderer.
 */
EXPORT HylaranaRender hylarana_renderer_create(HylaranaWindowHandle hwnd, HylaranaGraphicsBackend backend);

/**
 * Push the video frame into the renderer, which will update the window texture.
 */
EXPORT bool hylarana_renderer_on_video(HylaranaRender render, HylaranaVideoFrame* frame);

/**
 * Push the audio frame into the renderer, which will append to audio queue.
 */
EXPORT bool hylarana_renderer_on_audio(HylaranaRender render, HylaranaAudioFrame* frame);

/**
 * Destroy the window renderer.
 */
EXPORT void hylarana_renderer_destroy(HylaranaRender render);

typedef const void* HylaranaProperties;
typedef const void* HylaranaDiscovery;

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

#endif // MIRROR_H
