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

struct OutputCallback
{
    void (*video)(void* ctx, struct VideoFrame* frame);
    void (*audio)(void* ctx, struct AudioFrame* frame);
    void* ctx;
};

extern "C"
{
    // Releases all data associated with OBS and terminates the OBS context.
	EXPORT void capture_quit();
    // Initializes the OBS core context.
	EXPORT int capture_init(VideoInfo* video_info, AudioInfo* audio_info);
    // Enumerates all input sources.
    //
    // Callback function returns true to continue enumeration, or false to end 
    // enumeration.
	EXPORT struct DeviceList* capture_get_device_list(enum DeviceType type);
    // Sets the primary output source for a channel.
	EXPORT void capture_set_video_input(struct DeviceDescription* description);
    // Adds/removes a raw video/audio callback. Allows the ability to obtain raw video/audio
    // frames without necessarily using an output.
	EXPORT void* capture_set_output_callback(struct OutputCallback proc);
    EXPORT void capture_release_device_description(struct DeviceDescription* description);
	EXPORT void capture_release_device_list(struct DeviceList* list);
}

#endif /* capture_h */
