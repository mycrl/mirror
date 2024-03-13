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
#include <libavformat/avformat.h>
#include <libavutil/avutil.h>

EXPORT void init();
EXPORT const AVInputFormat* get_audio_device_next(const AVInputFormat* device);
EXPORT const AVInputFormat* get_video_device_next(const AVInputFormat* device);
EXPORT const char* get_device_name(const AVInputFormat* device);

#endif /* devices_h */
