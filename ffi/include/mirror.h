//
// mirror.h
// mirror
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
} VideoFormat;

typedef enum
{
    VIDEO_SUB_FORMAT_D3D11,
    VIDEO_SUB_FORMAT_SW,
} VideoSubFormat;

typedef struct
{
    VideoFormat format;
    VideoSubFormat sub_format;
    uint32_t width;
    uint32_t height;
    void* data[3];
    size_t linesize[3];
} VideoFrame;

typedef struct
{
    int sample_rate;
    uint32_t frames;
    int16_t* data;
} AudioFrame;

typedef enum
{
    SOURCE_TYPE_CAMERA,
    SOURCE_TYPE_SCREEN,
    SOURCE_TYPE_AUDIO,
} SourceType;

typedef struct
{
    size_t index;
    SourceType type;
    const char* id;
    const char* name;
    bool is_default;
} Source;

typedef struct
{
    Source* items;
    size_t capacity;
    size_t size;
} Sources;

typedef enum {
    VIDEO_DECODER_H264,
    VIDEO_DECODER_D3D11,
    VIDEO_DECODER_QSV,
    VIDEO_DECODER_CUDA,
    VIDEO_DECODER_VIDEOTOOLBOX,
} VideoDecoderType;

typedef enum 
{
    VIDEO_ENCODER_X264,
    VIDEO_ENCODER_QSV,
    VIDEO_ENCODER_CUDA,
    VIDEO_ENCODER_VIDEOTOOLBOX,
} VideoEncoderType;

typedef enum 
{
    RENDER_BACKEND_DX11,
    RENDER_BACKEND_WGPU,
} GraphicsBackend;

typedef struct
{
    /**
     * Video encoder settings, possible values are `h264_qsv`, `h264_nvenc`,
     * `libx264` and so on.
     */
    VideoEncoderType codec;
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
} VideoEncoderDescriptor;

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
} AudioEncoderDescriptor;

typedef struct
{
    Source* source;
    VideoEncoderDescriptor encoder;
} VideoDescriptor;

typedef struct
{
    Source* source;
    AudioEncoderDescriptor encoder;
} AudioDescriptor;

typedef struct
{
    VideoDescriptor* video;
    AudioDescriptor* audio;
    bool multicast;
} SenderDescriptor;

typedef struct
{
    /**
     * mirror server address.
     */
    const char* server;
    /**
     * Multicast address, e.g. `239.0.0.1`.
     */
    const char* multicast;
    /**
     * The size of the maximum transmission unit of the network, which is
     * related to the settings of network devices such as routers or switches,
     * the recommended value is 1400.
     */
    size_t mtu;
} MirrorDescriptor;

typedef const void* Mirror;
typedef const void* Sender;
typedef const void* Receiver;

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
    bool (*video)(void* ctx, VideoFrame* frame);
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
    bool (*audio)(void* ctx, AudioFrame* frame);
    /**
     * Callback when the sender is closed. This may be because the external
     * side actively calls the close, or the audio and video packets cannot be
     * sent (the network is disconnected), etc.
     */
    void (*close)(void* ctx);
    void* ctx;
} FrameSink;

/**
 * Because Linux does not have DllMain, you need to call it manually to achieve
 * similar behavior.
 */
#ifndef WIN32

/**
 * Initialize the environment, which must be initialized before using the SDK.
 */
EXPORT bool mirror_startup();

/**
 * Cleans up the environment when the SDK exits, and is recommended to be
 * called when the application exits.
 */
EXPORT void mirror_shutdown();

#endif // !WIN32

/**
 * Create mirror.
 */
EXPORT Mirror mirror_create(MirrorDescriptor options);

/**
 * Release mirror.
 */
EXPORT void mirror_destroy(Mirror mirror);

/**
 * Get capture sources.
 */
EXPORT Sources mirror_get_sources(SourceType kind);

/**
 * Because `Sources` are allocated internally, they also need to be released
 * internally.
 */
EXPORT void mirror_sources_destroy(Sources* sources);

/**
 * Create a sender, specify a bound NIC address, you can pass callback to
 * get the device screen or sound callback, callback can be null, if it is
 * null then it means no callback data is needed.
 */
EXPORT Sender mirror_create_sender(Mirror mirror, int id, SenderDescriptor options, FrameSink sink);

/**
 * Get whether the sender uses multicast transmission.
 */
EXPORT bool mirror_sender_get_multicast(Sender sender);

/**
 * Set whether the sender uses multicast transmission.
 */
EXPORT void mirror_sender_set_multicast(Sender sender, bool is_multicast);

/**
 * Close sender.
 */
EXPORT void mirror_sender_destroy(Sender sender);

/**
 * Create a receiver, specify a bound NIC address, you can pass callback to
 * get the sender's screen or sound callback, callback can not be null.
 */
EXPORT Receiver mirror_create_receiver(Mirror mirror, int id, VideoDecoderType codec, FrameSink sink);

/**
 * Close receiver.
 */
EXPORT void mirror_receiver_destroy(Receiver receiver);

typedef const void* WindowHandle;
typedef const void* Render;

#ifdef WIN32

/**
 * Raw window handle for Win32.
 * 
 * This variant is used on Windows systems.
 */
EXPORT WindowHandle create_window_handle_for_win32(HWND hwnd, uint32_t width, uint32_t height);

#endif // WIN32

#ifdef LINUX

/**
 * A raw window handle for Xlib.
 *
 * This variant is likely to show up anywhere someone manages to get X11
 * working that Xlib can be built for, which is to say, most (but not all)
 * Unix systems.
 */
EXPORT WindowHandle create_window_handle_for_xlib(uint32_t hwnd, void* display, int screen, uint32_t width, uint32_t height);

/**
 * A raw window handle for Xcb.
 *
 * This variant is likely to show up anywhere someone manages to get X11
 * working that XCB can be built for, which is to say, most (but not all)
 * Unix systems.
 */
EXPORT WindowHandle create_window_handle_for_xcb(uint32_t hwnd, void* display, int screen, uint32_t width, uint32_t height);

/**
 * A raw window handle for Wayland.
 *
 * This variant should be expected anywhere Wayland works, which is
 * currently some subset of unix systems.
 */
EXPORT WindowHandle create_window_handle_for_wayland(void* hwnd, void* display, uint32_t width, uint32_t height);

#endif

#ifdef MACOS

/**
 * A raw window handle for AppKit.
 *
 * This variant is likely to be used on macOS, although Mac Catalyst 
 * ($arch-apple-ios-macabi targets, which can notably use UIKit or AppKit) can 
 * also use it despite being target_os = "ios".
 */
EXPORT WindowHandle create_window_handle_for_appkit(void* view, uint32_t width, uint32_t height);

#endif

/**
 * Destroy the window handle.
 */
EXPORT void window_handle_destroy(WindowHandle hwnd);

/**
 * Creating a window renderer.
 */
EXPORT Render renderer_create(WindowHandle hwnd, GraphicsBackend backend);

/**
 * Push the video frame into the renderer, which will update the window texture.
 */
EXPORT bool renderer_on_video(Render render, VideoFrame* frame);

/**
 * Push the audio frame into the renderer, which will append to audio queue.
 */
EXPORT bool renderer_on_audio(Render render, AudioFrame* frame);

/**
 * Destroy the window renderer.
 */
EXPORT void renderer_destroy(Render render);

#endif // MIRROR_H
