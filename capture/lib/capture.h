//
//  capture.h
//  capture
//
//  Created by Panda on 2024/2/14.
//

#ifndef capture_h
#define capture_h
#pragma once

#ifdef WINDOWS
#define EXPORT __declspec(dllexport)
#else
#define EXPORT
#endif

#include <frame.h>

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
	enum DeviceType type;
	const char* id;
	const char* name;
	size_t index;
};

struct DeviceList
{
	struct DeviceDescription** devices;
	size_t size;
};

struct OutputCallback
{
    void (*video)(void* ctx, struct VideoFrame* frame);
    void (*audio)(void* ctx, struct AudioFrame* frame);
    void* ctx;
};

struct GetDeviceListResult
{
	int status;
	struct DeviceList* list;
};

extern "C"
{
    // Initializes the OBS core context.
	EXPORT int capture_init(VideoInfo* video_info, AudioInfo* audio_info);
    // Enumerates all input sources.
    //
    // Callback function returns true to continue enumeration, or false to end 
    // enumeration.
	EXPORT struct GetDeviceListResult capture_get_device_list(enum DeviceType type);
    // Sets the primary output source for a channel.
	EXPORT int capture_set_video_input(struct DeviceDescription* description);
    // Adds/removes a raw video/audio callback. Allows the ability to obtain raw video/audio
    // frames without necessarily using an output.
	EXPORT void* capture_set_output_callback(struct OutputCallback proc);
    EXPORT void capture_release_device_description(struct DeviceDescription* description);
	EXPORT void capture_release_device_list(struct DeviceList* list);
	// Start capturing audio and video data.
	EXPORT int capture_start();
	// Stop capturing audio and video data
	EXPORT void capture_stop();
}

#endif /* capture_h */
