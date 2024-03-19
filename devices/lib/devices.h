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

#include <stdint.h>
#include <stdbool.h>
#include <libavdevice/avdevice.h>
#include <libavformat/avformat.h>
#include <libavutil/avutil.h>
#include <libavcodec/packet.h>

typedef enum
{
    DeviceKindVideo = 0,
    DeviceKindAudio = 1
} DeviceKind;

typedef struct
{
    char* name;
    char* description;
    DeviceKind kind;
} Device;

typedef struct
{
    Device** items;
    size_t size;
} Devices;

typedef struct
{
    uint8_t* data;
    size_t size;
} Buffer;

typedef struct
{
    AVFormatContext* ctx;
    AVInputFormat* fmt;
    AVPacket* pkt;
    Buffer* buf;
} DeviceContext;

EXPORT void init();
EXPORT Devices get_audio_devices();
EXPORT Devices get_video_devices();
EXPORT void release_device(Device* device);
EXPORT void release_devices(Devices* devices);
EXPORT DeviceContext* open_device(Device* device);
EXPORT void release_device_context(DeviceContext* dctx);
EXPORT Buffer* device_read_packet(DeviceContext* dctx);

#endif /* devices_h */
