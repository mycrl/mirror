//
//  devices.h
//  devices
//
//  Created by Panda on 2024/2/14.
//

#ifndef devices_h
#define devices_h
#pragma once

#ifdef WIN32
#define EXPORT __declspec(dllexport)
#else
#define EXPORT
#endif

#include <stdbool.h>
#include <stdint.h>
#include <libavdevice/avdevice.h>

typedef struct
{
    const AVInputFormat** items;
    size_t size;
} Devices;

EXPORT void init();
EXPORT Devices get_audio_devices();
EXPORT Devices get_video_devices();
EXPORT void release_devices(Devices* devices);
EXPORT const char* get_device_name(const AVInputFormat* device);

#endif /* devices_h */
