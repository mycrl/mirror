//
//  capture.cpp
//  capture
//
//  Created by Mr.Panda on 2024/2/14.
//

#include "capture.h"

#include <format>
#include <thread>
#include <assert.h>
#include <libobs/obs.h>

#ifdef WIN32
#include <mfapi.h>
#include <mfidl.h>
#include <mfreadwrite.h>
#endif

#ifdef WIN32
#define OUTPUT_WINDOW_SOURCE	"window_capture"
#define OUTPUT_AUDIO_SOURCE		"wasapi_output_capture"
#define OUTPUT_MONITOR_SOURCE	"monitor_capture"
#define MONITOR_SOURCE_PROPERTY "monitor_id"
#define AUDIO_SOURCE_PROPERTY   "device_id"
#define WINDOW_SOURCE_PROPERTY  "window"
#elif LINUX
#define OUTPUT_AUDIO_SOURCE		"pulse_output_capture"
#endif

// global variable

struct Camera
{
    IMFAttributes* attributes;
    IMFActivate** devices;
    UINT32 devices_size;
    IMFMediaSource* source;
    IMFSourceReader* reader;
    IMFMediaType* kind;
    bool is_runing;
};

static struct
{
    struct obs_audio_info audio_info;
    struct obs_video_info video_info;
    obs_scene_t* scene;
    obs_source_t* monitor_source;
    obs_sceneitem_t* monitor_scene_item;
    obs_source_t* window_source;
    obs_sceneitem_t* window_scene_item;
    obs_source_t* audio_source;
    struct OutputCallback output_callback;
    struct VideoFrame video_frame;
    struct AudioFrame audio_frame;
    struct Camera camera;
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
    if (GLOBAL.output_callback.video == nullptr ||
        GLOBAL.output_callback.ctx == nullptr ||
        frame == nullptr)
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
    if (GLOBAL.output_callback.audio == nullptr ||
        GLOBAL.output_callback.ctx == nullptr ||
        data == nullptr)
    {
        return;
    }

    GLOBAL.audio_frame.data = data->data[0];
    GLOBAL.audio_frame.frames = data->frames;
    GLOBAL.output_callback.audio(GLOBAL.output_callback.ctx, &GLOBAL.audio_frame);
}

#ifdef WIN32
int init_camera()
{
    auto ret = MFStartup(MF_VERSION);
    if (!SUCCEEDED(ret))
    {
        return -1;
    }

    ret = MFCreateAttributes(&GLOBAL.camera.attributes, 1);
    if (!SUCCEEDED(ret))
    {
        return -2;
    }

    ret = GLOBAL.camera.attributes->SetGUID(MF_DEVSOURCE_ATTRIBUTE_SOURCE_TYPE,
                                            MF_DEVSOURCE_ATTRIBUTE_SOURCE_TYPE_VIDCAP_GUID);
    if (!SUCCEEDED(ret))
    {
        return -3;
    }

    return 0;
}

struct GetDeviceListResult enum_camera()
{
    DeviceList* list = new DeviceList{};
    list->devices = new DeviceDescription * [100];
    list->size = 0;

    GLOBAL.camera.devices_size = 0;
    auto ret = MFEnumDeviceSources(GLOBAL.camera.attributes,
                                   &GLOBAL.camera.devices,
                                   &GLOBAL.camera.devices_size);
    if (!SUCCEEDED(ret) || GLOBAL.camera.devices_size == 0)
    {
        return { -1, list };
    }

    for (UINT32 i = 0; i < GLOBAL.camera.devices_size; i++)
    {
        UINT32 size;
        WCHAR* wname = nullptr;
        ret = GLOBAL.camera.devices[i]->GetAllocatedString(MF_DEVSOURCE_ATTRIBUTE_FRIENDLY_NAME,
                                                           &wname,
                                                           &size);
        if (!SUCCEEDED(ret))
        {
            break;
        }

        int length = WideCharToMultiByte(CP_UTF8,
                                         0,
                                         wname,
                                         size,
                                         0,
                                         0,
                                         nullptr,
                                         nullptr);
        char* name = (char*)malloc((length + 1) * sizeof(char));
        if (!name)
        {
            continue;
        }

        WideCharToMultiByte(CP_UTF8,
                            0,
                            wname,
                            size,
                            name,
                            length,
                            nullptr,
                            nullptr);
        name[length] = '\0';
        CoTaskMemFree(wname);

        struct DeviceDescription* device = new DeviceDescription{};
        device->type = DeviceType::kDeviceTypeVideo;
        device->name = name;
        device->id = name;
        device->index = i;

        list->devices[list->size] = device;
        list->size++;
    }

    return { 0, list };
}

void frame_loop()
{
    for (; GLOBAL.camera.is_runing &&
         GLOBAL.camera.reader != nullptr &&
         GLOBAL.output_callback.video != nullptr;)
    {
        LONGLONG timestamp;
        DWORD index = 0, flags = 0;
        IMFSample* sample = nullptr;
        auto ret = GLOBAL.camera.reader->ReadSample((DWORD)MF_SOURCE_READER_FIRST_VIDEO_STREAM,
                                                    0,
                                                    &index,
                                                    &flags,
                                                    &timestamp,
                                                    &sample);
        assert(SUCCEEDED(ret));
        if (sample == nullptr)
        {
            continue;
        }

        IMFMediaBuffer* buffer = nullptr;
        ret = sample->ConvertToContiguousBuffer(&buffer);
        if (!SUCCEEDED(ret))
        {
            break;
        }

        DWORD len = 0;
        BYTE* frame = nullptr;
        ret = buffer->Lock(&frame, nullptr, &len);
        if (!SUCCEEDED(ret) || frame == nullptr)
        {
            break;
        }

        struct VideoFrame video_frame = {
            .rect = { 1280, 720 },
            .data = { frame, frame + (1280 * 720) },
            .linesize = { 1280, 1280 },
        };

        GLOBAL.output_callback.video(GLOBAL.output_callback.ctx, &video_frame);

        ret = buffer->Unlock();
        if (!SUCCEEDED(ret))
        {
            break;
        }

        buffer->Release();
        sample->Release();
        Sleep(1000 / GLOBAL.video_info.fps_num);
    }
}

int set_camera(struct DeviceDescription* description)
{
    auto ret = MFCreateDeviceSource(GLOBAL.camera.devices[description->index],
                                    &GLOBAL.camera.source);
    if (!SUCCEEDED(ret))
    {
        return -1;
    }

    ret = MFCreateSourceReaderFromMediaSource(GLOBAL.camera.source,
                                              GLOBAL.camera.attributes,
                                              &GLOBAL.camera.reader);
    if (!SUCCEEDED(ret))
    {
        return -2;
    }

    ret = MFCreateMediaType(&GLOBAL.camera.kind);
    if (!SUCCEEDED(ret))
    {
        return -3;
    }

    ret = GLOBAL.camera.kind->SetGUID(MF_MT_MAJOR_TYPE, MFMediaType_Video);
    if (!SUCCEEDED(ret))
    {
        return -4;
    }

    ret = GLOBAL.camera.kind->SetGUID(MF_MT_SUBTYPE, MFVideoFormat_NV12);
    if (!SUCCEEDED(ret))
    {
        return -5;
    }

    ret = MFSetAttributeSize(GLOBAL.camera.kind, MF_MT_FRAME_SIZE, 1280, 720);
    if (!SUCCEEDED(ret))
    {
        return -6;
    }

    ret = MFSetAttributeRatio(GLOBAL.camera.kind,
                              MF_MT_FRAME_RATE,
                              GLOBAL.video_info.fps_num,
                              1);
    if (!SUCCEEDED(ret))
    {
        return -7;
    }

    ret = GLOBAL.camera.reader->SetCurrentMediaType((DWORD)MF_SOURCE_READER_FIRST_VIDEO_STREAM,
                                                    nullptr,
                                                    GLOBAL.camera.kind);
    if (!SUCCEEDED(ret))
    {
        return -8;
    }

    GLOBAL.camera.is_runing = true;
    std::thread(frame_loop).detach();

    return 0;
}

void stop_camera()
{
    GLOBAL.camera.is_runing = false;

    if (GLOBAL.camera.kind != nullptr)
    {
        GLOBAL.camera.kind->Release();
        GLOBAL.camera.kind = nullptr;
    }

    if (GLOBAL.camera.reader != nullptr)
    {
        GLOBAL.camera.reader->Release();
        GLOBAL.camera.reader = nullptr;
    }

    if (GLOBAL.camera.source != nullptr)
    {
        GLOBAL.camera.source->Release();
        GLOBAL.camera.source = nullptr;
    }

    if (GLOBAL.camera.attributes != nullptr)
    {
        GLOBAL.camera.attributes->Release();
        GLOBAL.camera.attributes = nullptr;
    }

    for (auto i = 0; i < GLOBAL.camera.devices_size; i++)
    {
        GLOBAL.camera.devices[i]->Release();
    }

    GLOBAL.camera.devices = nullptr;
    MFShutdown();
}
#endif

// export api

void* capture_set_output_callback(struct OutputCallback proc)
{
    void* previous_ctx = GLOBAL.output_callback.ctx;
    GLOBAL.output_callback = proc;
    return previous_ctx;
}

int capture_init(VideoInfo* video_info, AudioInfo* audio_info)
{
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

int capture_start()
{
    if (obs_initialized())
    {
        return -1;
    }

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

    // default scene
    GLOBAL.scene = obs_scene_create("Default");
    if (GLOBAL.scene == nullptr)
    {
        return -5;
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

    struct video_scale_info video_scale_info;
    video_scale_info.format = VIDEO_FORMAT_NV12;
    video_scale_info.width = GLOBAL.video_info.base_width;
    video_scale_info.height = GLOBAL.video_info.base_height;
    obs_add_raw_video_callback(&video_scale_info, raw_video_callback, nullptr);

    struct audio_convert_info audio_convert_info;
    audio_convert_info.speakers = SPEAKERS_MONO;
    audio_convert_info.format = AUDIO_FORMAT_16BIT;
    audio_convert_info.samples_per_sec = GLOBAL.audio_info.samples_per_sec;
    obs_add_raw_audio_callback(1, &audio_convert_info, raw_audio_callback, nullptr);

    obs_set_output_source(0, obs_scene_get_source(GLOBAL.scene));
    obs_set_output_source(1, GLOBAL.audio_source);

    init_camera();
}

void capture_stop()
{
    if (!obs_initialized())
    {
        return;
    }

    obs_set_output_source(0, nullptr);
    obs_set_output_source(1, nullptr);

    obs_remove_raw_video_callback(raw_video_callback, nullptr);
    obs_remove_raw_audio_callback(1, raw_audio_callback, nullptr);

    if (GLOBAL.monitor_source != nullptr)
    {
        obs_source_release(GLOBAL.monitor_source);
    }

    if (GLOBAL.window_source != nullptr)
    {
        obs_source_release(GLOBAL.window_source);
    }

    if (GLOBAL.audio_source != nullptr)
    {
        obs_source_release(GLOBAL.audio_source);
    }

    if (GLOBAL.scene != nullptr)
    {
        obs_scene_release(GLOBAL.scene);
    }

    obs_shutdown();
    stop_camera();
}

int capture_set_video_input(struct DeviceDescription* description)
{
    if (description->type == DeviceType::kDeviceTypeVideo)
    {
        return set_camera(description);
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
    DeviceList* list = new DeviceList{};
    list->devices = new DeviceDescription * [100];
    list->size = 0;

    std::string key;
    obs_source_t* source = nullptr;
    if (type == DeviceType::kDeviceTypeVideo)
    {
        return enum_camera();
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
                device->index = i;
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
