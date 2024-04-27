//
//  devices.h
//  devices
//
//  Created by Panda on 2024/2/14.
//

#ifndef devices_h
#define devices_h
#pragma once

#ifdef WINDOWS
#define EXPORT __declspec(dllexport)
#else
#define EXPORT
#endif

#include <libobs/obs.h>
#include <stdio.h>
#include <frame.h>

struct VideoInfo
{
    uint8_t fps;
    uint32_t width;
    uint32_t height;
};

enum DeviceType
{
    kDeviceTypeVideo,
    kDeviceTypeAudio,
    kDeviceTypeScreen,
};

struct DeviceDescription
{
    enum DeviceType type;
    const char* id;
    const char* name;
};

struct DeviceList
{
    struct DeviceDescription** devices;
    size_t size;
};

typedef void (*VideoOutputCallback)(void* ctx, struct VideoFrame* frame);

extern "C"
{
EXPORT void _quit();
EXPORT int _init(VideoInfo* info);
EXPORT struct DeviceList _get_device_list(enum DeviceType type);
EXPORT void _release_device_description(struct DeviceDescription* description);
EXPORT void _set_video_input(struct DeviceDescription* description);
EXPORT void* _set_video_output_callback(VideoOutputCallback proc, void* ctx);
}

#endif /* devices_h */
