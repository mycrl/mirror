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

typedef struct
{
    uint8_t fps;
    uint32_t width;
    uint32_t height;
} VideoInfo;

typedef struct
{
    obs_scene_t* scene;
    obs_source_t* video_source;
    obs_sceneitem_t* video_scene_item;
} DeviceManager;

typedef enum
{
    kDeviceTypeVideo,
    kDeviceTypeAudio,
    kDeviceTypeScreen,
} DeviceType;

typedef struct
{
    DeviceType type;
    const char* id;
    const char* name;
} DeviceDescription;

typedef struct
{
    DeviceDescription** devices;
    size_t size;
} DeviceList;

typedef void (*VideoOutputCallback)(void* ctx, VideoFrame frame);

EXPORT int _init(VideoInfo* info);
EXPORT DeviceManager* _create_device_manager();
EXPORT void _device_manager_release(DeviceManager* manager);
EXPORT DeviceList _get_device_list(DeviceManager* manager, DeviceType type);
EXPORT void _release_device_description(DeviceDescription* description);
EXPORT void _set_video_input(DeviceManager* manager, DeviceDescription* description, VideoInfo* info);
EXPORT void _set_video_output_callback(VideoOutputCallback proc, FrameRect rect, void* ctx);

#endif /* devices_h */
