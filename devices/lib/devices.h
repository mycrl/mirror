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
#include <libavcodec/avcodec.h>
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
    const AVInputFormat* fmt;
} DeviceInfo;

typedef struct
{
    DeviceInfo** items;
    size_t size;
} Devices;

typedef struct
{
    int format;
    uint32_t width;
    uint32_t height;
    uint8_t** planes;
    uint32_t* linesizes;
} VideoFrame;

typedef struct
{
    const AVInputFormat* fmt;
    AVFormatContext* ctx;
    AVPacket* pkt;
    AVFrame* frame;
    int stream_idx;
    const AVCodec* codec;
    AVCodecContext* codec_ctx;
    VideoFrame* video_frame;
} Device;

typedef struct
{
    uint32_t width;
    uint32_t height;
    uint8_t frame_rate;
} DeviceConstraint;

EXPORT void init();
EXPORT Devices get_audio_devices();
EXPORT Devices get_video_devices();
EXPORT void release_device_info(DeviceInfo* device);
EXPORT void release_devices(Devices* devices);
EXPORT Device* open_device(DeviceInfo* device, DeviceConstraint constraint);
EXPORT void release_device(Device* device);
EXPORT int device_advance(Device* device);
EXPORT VideoFrame* device_get_frame(Device* device);

#endif /* devices_h */
