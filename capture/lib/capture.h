//
//  capture.h
//  capture
//
//  Created by Panda on 2024/2/14.
//

#ifndef capture_h
#define capture_h
#pragma once

#ifndef EXPORT
#ifdef WINDOWS
#define EXPORT __declspec(dllexport)
#else
#define EXPORT
#endif
#endif

extern "C"
{
#include <frame.h>
}

enum CaptureMethod
{
    GDI,
    DXGI,
    WGC,
};

struct VideoInfo
{
    uint8_t fps;
    uint32_t width;
    uint32_t height;
};

struct AudioInfo
{
    uint32_t samples_per_sec;
};

enum DeviceType
{
    kDeviceTypeVideo,
    kDeviceTypeAudio,
    kDeviceTypeScreen,
    kDeviceTypeWindow,
};

struct DeviceDescription
{
    DeviceType type;
    const char* id;
    const char* name;
};

struct DeviceList
{
    DeviceDescription** devices;
    size_t size;
};

struct OutputCallback
{
    void (*video)(void* ctx, VideoFrame* frame);
    void (*audio)(void* ctx, AudioFrame* frame);
    void* ctx;
};

struct GetDeviceListResult
{
    int status;
    DeviceList* list;
};

typedef void (*Logger)(int level, const char* message, void* ctx);

struct CaptureSettings
{
    CaptureMethod method;
};

extern "C"
{
    EXPORT void* capture_remove_logger();
    EXPORT void capture_set_logger(Logger logger, void* ctx);
    // Initializes the OBS core context.
    EXPORT void capture_init(VideoInfo* video_info, AudioInfo* audio_info);
    // Enumerates all input sources.
    //
    // Callback function returns true to continue enumeration, or false to end 
    // enumeration.
    EXPORT GetDeviceListResult capture_get_device_list(DeviceType type, CaptureSettings* settings);
    // Sets the primary output source for a channel.
    EXPORT int capture_set_input(DeviceDescription* description, CaptureSettings* settings);
    // Adds/removes a raw video/audio callback. Allows the ability to obtain raw video/audio
    // frames without necessarily using an output.
    EXPORT void* capture_set_output_callback(OutputCallback proc);
    EXPORT void capture_release_device_description(DeviceDescription* description);
    EXPORT void capture_release_device_list(DeviceList* list);
    // Start capturing audio and video data.
    EXPORT int capture_start();
    // Stop capturing audio and video data
    EXPORT void capture_stop();
}

#endif /* capture_h */
