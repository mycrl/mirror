//
//  capture.cpp
//  capture
//
//  Created by Mr.Panda on 2024/2/14.
//

#include "capture.h"

#include <format>
#include <libobs/obs.h>

#ifdef WIN32
#define OUTPUT_WINDOW_SOURCE	"window_capture"
#define OUTPUT_AUDIO_SOURCE		"wasapi_output_capture"
#define OUTPUT_MONITOR_SOURCE	"monitor_capture"
#define OUTPUT_VIDEO_SOURCE		"dshow_input"
#define VIDEO_SOURCE_PROPERTY	"video_device_id"
#define MONITOR_SOURCE_PROPERTY "monitor_id"
#define AUDIO_SOURCE_PROPERTY   "device_id"
#define WINDOW_SOURCE_PROPERTY  "window"
#elif LINUX
#define OUTPUT_AUDIO_SOURCE		"pulse_output_capture"
#endif

// global variable

static struct
{
	struct obs_audio_info audio_info;
	struct obs_video_info video_info;
	obs_scene_t* scene;
	obs_source_t* video_source;
	obs_sceneitem_t* video_scene_item;
	obs_source_t* monitor_source;
	obs_sceneitem_t* monitor_scene_item;
	obs_source_t* window_source;
	obs_sceneitem_t* window_scene_item;
	obs_source_t* audio_source;
	struct OutputCallback output_callback;
	struct VideoFrame video_frame;
	struct AudioFrame audio_frame;
} GLOBAL = {};

// update settings

void set_video_item_scale(obs_sceneitem_t* item)
{
	obs_sceneitem_set_scale_filter(item, OBS_SCALE_BILINEAR);

	float width = float(GLOBAL.video_info.base_width);
	float height = float(GLOBAL.video_info.base_height);

	obs_transform_info info;
	info.crop_to_bounds = obs_sceneitem_get_bounds_crop(item);
	info.alignment = OBS_ALIGN_LEFT | OBS_ALIGN_TOP;
	info.bounds_type = OBS_BOUNDS_SCALE_INNER;
	info.bounds_alignment = OBS_ALIGN_CENTER;
	info.rot = 0.0f;

	vec2_set(&info.pos, 0.0f, 0.0f);
	vec2_set(&info.scale, 1.0f, 1.0f);
	vec2_set(&info.bounds, width, height);

	obs_sceneitem_set_info2(item, &info);
}

void update_video_settings(struct DeviceDescription* description)
{
	obs_data_t* settings = obs_data_create();
	obs_data_apply(settings, obs_source_get_settings(GLOBAL.video_source));

#ifdef WIN32
	std::string resolution = std::format("{}x{}",
										 GLOBAL.video_info.base_width,
										 GLOBAL.video_info.base_height);

	obs_data_set_int(settings, "res_type", 1);
	obs_data_set_bool(settings, "hw_decode", true);
	obs_data_set_string(settings, "resolution", resolution.c_str());
	obs_data_set_string(settings, "video_device_id", description->id);
#endif

	obs_source_update(GLOBAL.video_source, settings);
	obs_sceneitem_set_visible(GLOBAL.video_scene_item, true);
	obs_sceneitem_set_visible(GLOBAL.window_scene_item, false);
	obs_sceneitem_set_visible(GLOBAL.monitor_scene_item, false);

	obs_data_release(settings);
}

void update_monitor_settings(struct DeviceDescription* description)
{
	obs_data_t* settings = obs_data_create();
	obs_data_apply(settings, obs_source_get_settings(GLOBAL.monitor_source));

#ifdef WIN32
	obs_data_set_bool(settings, "force_sdr", true);
	obs_data_set_bool(settings, "compatibility", true);
	obs_data_set_bool(settings, "capture_cursor", false);
	obs_data_set_int(settings, "method", 2 /* METHOD_WGC */); // windows 10+ only
	obs_data_set_string(settings, "monitor_id", description->id);
#endif

	obs_source_update(GLOBAL.monitor_source, settings);
	obs_sceneitem_set_visible(GLOBAL.monitor_scene_item, true);
	obs_sceneitem_set_visible(GLOBAL.video_scene_item, false);
	obs_sceneitem_set_visible(GLOBAL.window_scene_item, false);

	obs_data_release(settings);
}

void update_window_settings(struct DeviceDescription* description)
{
	obs_data_t* settings = obs_data_create();
	obs_data_apply(settings, obs_source_get_settings(GLOBAL.window_source));

#ifdef WIN32
	obs_data_set_bool(settings, "force_sdr", true);
	obs_data_set_bool(settings, "compatibility", true);
	obs_data_set_bool(settings, "capture_cursor", false);
	obs_data_set_int(settings, "method", 2 /* METHOD_WGC */); // windows 10+ only
	obs_data_set_string(settings, "window", description->id);
#endif

	obs_source_update(GLOBAL.window_source, settings);
	obs_sceneitem_set_visible(GLOBAL.window_scene_item, true);
	obs_sceneitem_set_visible(GLOBAL.video_scene_item, false);
	obs_sceneitem_set_visible(GLOBAL.monitor_scene_item, false);

	obs_data_release(settings);
}

void update_audio_settings(struct DeviceDescription* description)
{
	obs_data_t* settings = obs_data_create();
	obs_data_apply(settings, obs_source_get_settings(GLOBAL.audio_source));

#ifdef WIN32
	obs_data_set_string(settings, "device_id", description->id);
#endif

	obs_source_update(GLOBAL.audio_source, settings);
	obs_data_release(settings);
}

// raw frame callback

void raw_video_callback(void* _, struct video_data* frame)
{
	if (GLOBAL.output_callback.video == nullptr || GLOBAL.output_callback.ctx == nullptr || frame == nullptr)
	{
		return;
	}

	GLOBAL.video_frame.data[0] = frame->data[0];
	GLOBAL.video_frame.data[1] = frame->data[1];
	GLOBAL.video_frame.linesize[0] = (size_t)frame->linesize[0];
	GLOBAL.video_frame.linesize[1] = (size_t)frame->linesize[1];
	GLOBAL.output_callback.video(GLOBAL.output_callback.ctx, &GLOBAL.video_frame);
}

void raw_audio_callback(void* _, size_t mix_idx, struct audio_data* data)
{
	if (GLOBAL.output_callback.audio == nullptr || GLOBAL.output_callback.ctx == nullptr  || data == nullptr)
	{
		return;
	}

	GLOBAL.audio_frame.frames = data->frames;
	GLOBAL.audio_frame.data[0] = data->data[0];
	GLOBAL.audio_frame.data[1] = data->data[1];
	GLOBAL.output_callback.audio(GLOBAL.output_callback.ctx, &GLOBAL.audio_frame);
}

int capture_initialization()
{
	if (!obs_startup("en-US", nullptr, nullptr))
	{
		return -2;
	}

	if (obs_reset_video(&GLOBAL.video_info) != OBS_VIDEO_SUCCESS)
	{
		return -3;
	}

	if (!obs_reset_audio(&GLOBAL.audio_info))
	{
		return -4;
	}

	// load all modules
	obs_load_all_modules();
	obs_post_load_modules();

	struct video_scale_info video_scale_info;
	video_scale_info.width = GLOBAL.video_info.base_width;
	video_scale_info.height = GLOBAL.video_info.base_height;
	video_scale_info.format = VIDEO_FORMAT_NV12;
	obs_add_raw_video_callback(&video_scale_info, raw_video_callback, nullptr);

	struct audio_convert_info audio_convert_info;
	audio_convert_info.speakers = SPEAKERS_MONO;
	audio_convert_info.format = AUDIO_FORMAT_16BIT;
	audio_convert_info.samples_per_sec = GLOBAL.audio_info.samples_per_sec;
	obs_add_raw_audio_callback(1, &audio_convert_info, raw_audio_callback, nullptr);

	// default scene
	GLOBAL.scene = obs_scene_create("Default");
	if (GLOBAL.scene == nullptr)
	{
		return -5;
	}
	else
	{
		obs_set_output_source(0, obs_scene_get_source(GLOBAL.scene));
	}

	// window source
	GLOBAL.window_source = obs_source_create(OUTPUT_WINDOW_SOURCE,
											 "WindowCapture",
											 nullptr,
											 nullptr);
	if (GLOBAL.window_source == nullptr)
	{
		return -6;
	}

	GLOBAL.window_scene_item = obs_scene_add(GLOBAL.scene, GLOBAL.window_source);
	if (GLOBAL.window_scene_item == nullptr)
	{
		return -7;
	}
	else
	{
		set_video_item_scale(GLOBAL.window_scene_item);
	}

	// monitor source
	GLOBAL.monitor_source = obs_source_create(OUTPUT_MONITOR_SOURCE,
											  "MonitorCapture",
											  nullptr,
											  nullptr);
	if (GLOBAL.monitor_source == nullptr)
	{
		return -8;
	}

	GLOBAL.monitor_scene_item = obs_scene_add(GLOBAL.scene, GLOBAL.monitor_source);
	if (GLOBAL.monitor_scene_item == nullptr)
	{
		return -9;
	}
	else
	{
		set_video_item_scale(GLOBAL.monitor_scene_item);
	}

	// video source
	GLOBAL.video_source = obs_source_create(OUTPUT_VIDEO_SOURCE,
											"VideoCaptureDevice",
											nullptr,
											nullptr);
	if (GLOBAL.video_source == nullptr)
	{
		return -10;
	}

	GLOBAL.video_scene_item = obs_scene_add(GLOBAL.scene, GLOBAL.video_source);
	if (GLOBAL.video_scene_item == nullptr)
	{
		return -11;
	}
	else
	{
		set_video_item_scale(GLOBAL.video_scene_item);
	}

	// create default audio source
	obs_data_t* audio_settings = obs_data_create();
	obs_data_set_string(audio_settings, AUDIO_SOURCE_PROPERTY, "default");
	GLOBAL.audio_source = obs_source_create(OUTPUT_AUDIO_SOURCE,
											"AudioDevice",
											audio_settings,
											nullptr);
	if (GLOBAL.audio_source == nullptr)
	{
		return -12;
	}
	else
	{
		obs_set_output_source(1, GLOBAL.audio_source);
	}

	return 0;
}

// export api

void* capture_set_output_callback(struct OutputCallback proc)
{
	void* previous_ctx = GLOBAL.output_callback.ctx;
	GLOBAL.output_callback = proc;
	return previous_ctx;
}

int capture_init(VideoInfo* video_info, AudioInfo* audio_info)
{
	if (obs_initialized())
	{
		return -1;
	}

#ifdef WIN32
	GLOBAL.video_info.graphics_module = "libobs-d3d11";
#endif

	GLOBAL.video_info.fps_num = video_info->fps;
	GLOBAL.video_info.fps_den = 1;
	GLOBAL.video_info.gpu_conversion = true;
	GLOBAL.video_info.base_width = video_info->width;
	GLOBAL.video_info.base_height = video_info->height;
	GLOBAL.video_info.output_width = video_info->width;
	GLOBAL.video_info.output_height = video_info->height;
	GLOBAL.video_info.colorspace = VIDEO_CS_DEFAULT;
	GLOBAL.video_info.range = VIDEO_RANGE_DEFAULT;
	GLOBAL.video_info.scale_type = OBS_SCALE_BILINEAR;
	GLOBAL.video_info.output_format = VIDEO_FORMAT_NV12;
	GLOBAL.video_info.adapter = 0;
	GLOBAL.video_frame.rect.width = video_info->width;
	GLOBAL.video_frame.rect.height = video_info->height;
	GLOBAL.audio_info.samples_per_sec = audio_info->samples_per_sec;
	GLOBAL.audio_info.speakers = SPEAKERS_STEREO;

	return 0;
}

void capture_quit()
{
	if (GLOBAL.video_source != nullptr)
	{
		obs_source_release(GLOBAL.video_source);
	}

	if (GLOBAL.video_scene_item != nullptr)
	{
		obs_sceneitem_release(GLOBAL.video_scene_item);
	}

	if (GLOBAL.monitor_source != nullptr)
	{
		obs_source_release(GLOBAL.monitor_source);
	}

	if (GLOBAL.monitor_scene_item != nullptr)
	{
		obs_sceneitem_release(GLOBAL.monitor_scene_item);
	}

	if (GLOBAL.window_source != nullptr)
	{
		obs_source_release(GLOBAL.window_source);
	}

	if (GLOBAL.window_scene_item != nullptr)
	{
		obs_sceneitem_release(GLOBAL.window_scene_item);
	}

	if (GLOBAL.audio_source != nullptr)
	{
		obs_source_release(GLOBAL.audio_source);
	}

    if (GLOBAL.scene != nullptr)
	{
		obs_scene_release(GLOBAL.scene);
	}

	if (obs_initialized())
	{
		obs_shutdown();
	}
}

int capture_set_video_input(struct DeviceDescription* description)
{
	if (!obs_initialized())
	{
		int status = capture_initialization();
		if (status != 0)
		{
			return status;
		}
	}

	if (description->type == DeviceType::kDeviceTypeVideo)
	{
		update_video_settings(description);
	}
	else if (description->type == DeviceType::kDeviceTypeScreen)
	{
		update_monitor_settings(description);
	}
	else if (description->type == DeviceType::kDeviceTypeAudio)
	{
		update_audio_settings(description);
	}
	else if (description->type == DeviceType::kDeviceTypeWindow)
	{
		update_window_settings(description);
	}

	return 0;
}

struct GetDeviceListResult capture_get_device_list(enum DeviceType type)
{
	if (!obs_initialized())
	{
		int status = capture_initialization();
		if (status != 0)
		{
			return { status, nullptr };
		}
	}

	DeviceList* list = new DeviceList{};
	list->devices = new DeviceDescription * [100];
	list->size = 0;

	std::string key;
	obs_source_t* source = nullptr;
	if (type == DeviceType::kDeviceTypeVideo)
	{
		source = GLOBAL.video_source;
		key = VIDEO_SOURCE_PROPERTY;

	}
	else if (type == DeviceType::kDeviceTypeScreen)
	{
		source = GLOBAL.monitor_source;
		key = MONITOR_SOURCE_PROPERTY;
	}
	else if (type == DeviceType::kDeviceTypeAudio)
	{
		source = GLOBAL.audio_source;
		key = AUDIO_SOURCE_PROPERTY;
	}
	else if (type == DeviceType::kDeviceTypeWindow)
	{
		source = GLOBAL.window_source;
		key = WINDOW_SOURCE_PROPERTY;
	}

	obs_properties_t* properties = obs_source_properties(source);
	obs_property_t* property = obs_properties_first(properties);
	while (property)
	{
		std::string name = std::string(obs_property_name(property));
		if (name == key)
		{
			for (size_t i = 0; i < obs_property_list_item_count(property); i++)
			{
				// default audio device is used, ignore default audio
				const char* id = obs_property_list_item_string(property, i);
				if (type == DeviceType::kDeviceTypeAudio && std::string(std::move(id)) == "default")
				{
					continue;
				}

				struct DeviceDescription* device = new DeviceDescription{};
				device->name = obs_property_list_item_name(property, i);
				device->type = type;
				device->id = id;

				list->devices[list->size] = device;
				list->size++;
			}
		}

		obs_property_next(&property);
	}

	return { 0, list };
}

void capture_release_device_description(struct DeviceDescription* description)
{
	delete description;
}

void capture_release_device_list(struct DeviceList* list)
{
	delete list->devices;
	delete list;
}
