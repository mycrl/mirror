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

#include <frame.h>
#include <stdint.h>

typedef enum 
{
    Video,
    Audio,
    Screen,
    Window,
} DeviceKind;

typedef enum
{
    GDI,
    DXGI,
    WGC,
} CaptureMethod;

typedef struct
{
    CaptureMethod method;
} CaptureSettings;

typedef struct
{
    /**
     * Video encoder settings, possible values are `h264_qsv`, `h264_nvenc`,
     * `libx264` and so on.
     */
    char* encoder;
    /**
     * Video decoder settings, possible values are `h264_qsv`, `h264_cuvid`,
     * `h264`, etc.
     */
    char* decoder;
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
} VideoOptions;

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
} AudioOptions;

typedef struct
{
    /**
     * Video Codec Configuration.
     */
    VideoOptions video;
    /**
     * Audio Codec Configuration.
     */
    AudioOptions audio;
    /**
     * mirror server address.
     */
    char* server;
    /**
     * Multicast address, e.g. `239.0.0.1`.
     */
    char* multicast;
    /**
     * The size of the maximum transmission unit of the network, which is
     * related to the settings of network devices such as routers or switches,
     * the recommended value is 1400.
     */
    size_t mtu;
} MirrorOptions;

typedef struct
{
    const void* description;
} Device;

typedef struct
{
    /**
     * device list.
     */
    const Device* devices;
    /**
     * device vector capacity.
     */
    size_t capacity;
    /**
     * device vector size.
     */
    size_t size;
} Devices;

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
    * Automatically search for encoders, limited hardware, fallback to software
    * implementation if hardware acceleration unit is not found.
    */
EXPORT const char* mirror_find_video_encoder();
/**
    * Automatically search for decoders, limited hardware, fallback to software
    * implementation if hardware acceleration unit is not found.
    */
EXPORT const char* mirror_find_video_decoder();
/**
    * Cleans up the environment when the SDK exits, and is recommended to be
    * called when the application exits.
    */
EXPORT void mirror_quit();
/**
    * Initialize the environment, which must be initialized before using the SDK.
    */
EXPORT bool mirror_init(MirrorOptions options);
/**
    * Get device name.
    */
EXPORT const char* mirror_get_device_name(const Device* device);
/**
    * Get device kind.
    */
EXPORT DeviceKind mirror_get_device_kind(const Device* device);
/**
    * Get devices from device manager.
    */
EXPORT Devices mirror_get_devices(DeviceKind kind, CaptureSettings* settings);
/**
    * Release devices.
    */
EXPORT void mirror_devices_destroy(Devices* devices);
/**
    * Setting up an input device, repeated settings for the same type of device
    * will overwrite the previous device.
    */
EXPORT bool mirror_set_input_device(const Device* device, CaptureSettings* settings);
/**
    * Start capturing audio and video data.
    */
EXPORT int mirror_start_capture();
/**
    * Stop capturing audio and video data.
    */
EXPORT void mirror_stop_capture();
/**
    * Create mirror.
    */
EXPORT Mirror mirror_create();
/**
    * Release mirror.
    */
EXPORT void mirror_destroy(Mirror mirror);
/**
    * Create a sender, specify a bound NIC address, you can pass callback to
    * get the device screen or sound callback, callback can be null, if it is
    * null then it means no callback data is needed.
    */
EXPORT Sender mirror_create_sender(Mirror mirror, int id, FrameSink sink);
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
EXPORT Receiver mirror_create_receiver(Mirror mirror, int id, FrameSink sink);
/**
    * Close receiver.
    */
EXPORT void mirror_receiver_destroy(Receiver receiver);

#endif // MIRROR_H
