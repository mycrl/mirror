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

typedef struct
{
    uint8_t fps;
    uint32_t width;
    uint32_t height;
    enum video_format format;
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

typedef void (*VideoOutputCallback)(void* ctx, struct video_data* frame);

EXPORT int init(VideoInfo* info);
EXPORT DeviceManager* create_device_manager();
EXPORT void device_manager_release(DeviceManager* manager);
EXPORT DeviceList get_device_list(DeviceManager* manager, DeviceType type);
EXPORT void release_device_description(DeviceDescription* description);
EXPORT void set_video_input(DeviceManager* manager, DeviceDescription* description, VideoInfo* info);
EXPORT void set_video_output_callback(VideoOutputCallback proc, void* ctx);

#endif /* devices_h */
