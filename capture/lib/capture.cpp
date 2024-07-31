//
//  capture.cpp
//  capture
//
//  Created by Mr.Panda on 2024/2/14.
//

#include "./capture.h"
#include "./camera.h"
#include "./desktop.h"

#include <thread>
#include <mutex>
#include <string>
#include <libobs/obs.h>

#ifdef WIN32
#define OUTPUT_AUDIO_SOURCE		"wasapi_output_capture"
#define OUTPUT_WINDOW_SOURCE	"window_capture"
#define OUTPUT_MONITOR_SOURCE	"monitor_capture"
#define MONITOR_SOURCE_PROPERTY "monitor_id"
#define WINDOW_SOURCE_PROPERTY  "window"
#define AUDIO_SOURCE_PROPERTY   "device_id"
#else
#define OUTPUT_AUDIO_SOURCE		"pulse_output_capture"
#define OUTPUT_WINDOW_SOURCE    "xcomposite_input"
#define OUTPUT_MONITOR_SOURCE   "xshm_input"
#define MONITOR_SOURCE_PROPERTY "screen"
#define WINDOW_SOURCE_PROPERTY  "capture_window"
#define AUDIO_SOURCE_PROPERTY   "device_id"
#endif

// global variable

struct
{
    bool initialized = false;
    bool allow_obs = true;
    Logger logger = nullptr;
    void* logger_ctx = nullptr;
    obs_audio_info audio_info;
    obs_video_info video_info;
    obs_scene_t* scene;
    obs_source_t* monitor_source;
    obs_sceneitem_t* monitor_scene_item;
    obs_source_t* window_source;
    obs_sceneitem_t* window_scene_item;
    obs_source_t* audio_source;
    OutputCallback output_callback;
    VideoFrame video_frame;
    AudioFrame audio_frame;
#ifdef WIN32
    CameraCapture* camera_capture = nullptr;
    GDICapture* gdi_capture = nullptr;
#endif
} GLOBAL = {};
std::mutex GLOBAL_MUTEX;

void logger_proc(int level, const char* message, va_list args, void* _)
{
    if (GLOBAL.logger == nullptr)
    {
        return;
    }

    char str[8192];
    vsnprintf(str, sizeof(str), message, args);
    GLOBAL.logger(level, str, GLOBAL.logger_ctx);
}

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

void update_monitor_settings(DeviceDescription* description,
                             CaptureSettings* config)
{
    obs_data_t* settings = obs_data_create();
    obs_data_apply(settings, obs_source_get_settings(GLOBAL.monitor_source));

#ifdef WIN32
    SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);

    obs_data_set_bool(settings, "force_sdr", true);
    obs_data_set_bool(settings, "compatibility", true);
    obs_data_set_bool(settings, "capture_cursor", false);
    obs_data_set_string(settings, "monitor_id", description->id);
    obs_data_set_int(settings, "method", config ? config->method : CaptureMethod::WGC);
#endif

    obs_source_update(GLOBAL.monitor_source, settings);
    obs_sceneitem_set_visible(GLOBAL.monitor_scene_item, true);
    obs_sceneitem_set_visible(GLOBAL.window_scene_item, false);

    obs_data_release(settings);
}

void update_window_settings(DeviceDescription* description)
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

void update_audio_settings(DeviceDescription* description)
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

void raw_video_callback(void* _, video_data* frame)
{
    if (!GLOBAL_MUTEX.try_lock())
    {
        return;
    }

    if (GLOBAL.allow_obs && GLOBAL.initialized)
    {
        if (GLOBAL.output_callback.video != nullptr &&
            GLOBAL.output_callback.ctx != nullptr &&
            frame != nullptr)
        {
            GLOBAL.video_frame.data[0] = frame->data[0];
            GLOBAL.video_frame.data[1] = frame->data[1];
            GLOBAL.video_frame.linesize[0] = (size_t)frame->linesize[0];
            GLOBAL.video_frame.linesize[1] = (size_t)frame->linesize[1];
            GLOBAL.output_callback.video(GLOBAL.output_callback.ctx, &GLOBAL.video_frame);
        }
    }

    GLOBAL_MUTEX.unlock();
}

void raw_audio_callback(void* _, size_t mix_idx, audio_data* data)
{
    if (!GLOBAL_MUTEX.try_lock())
    {
        return;
    }

    if (GLOBAL.allow_obs && GLOBAL.initialized)
    {
        if (GLOBAL.output_callback.audio != nullptr &&
            GLOBAL.output_callback.ctx != nullptr &&
            data != nullptr)
        {
            GLOBAL.audio_frame.data = data->data[0];
            GLOBAL.audio_frame.frames = data->frames;
            GLOBAL.output_callback.audio(GLOBAL.output_callback.ctx, &GLOBAL.audio_frame);
        }
    }

    GLOBAL_MUTEX.unlock();
}

// export api

void* capture_set_output_callback(OutputCallback proc)
{
    blog(LOG_INFO, "CaptureModule: capture set output callback");
    std::lock_guard<std::mutex> lock_guard(GLOBAL_MUTEX);

    void* previous_ctx = GLOBAL.output_callback.ctx;
    GLOBAL.output_callback = proc;
    blog(LOG_INFO, "CaptureModule: capture set output callback done");

    return previous_ctx;
}

void capture_init(VideoInfo* video_info, AudioInfo* audio_info)
{
    base_set_log_handler(logger_proc, nullptr);

    blog(LOG_INFO, "CaptureModule: capture init");
    std::lock_guard<std::mutex> lock_guard(GLOBAL_MUTEX);

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
}

int capture_start()
{
    blog(LOG_INFO, "CaptureModule: capture start");
    std::lock_guard<std::mutex> lock_guard(GLOBAL_MUTEX);

    if (GLOBAL.initialized)
    {
        return -1;
    }

    blog(LOG_INFO, "CaptureModule: capture not initialized");

#ifdef WIN32
    GLOBAL.camera_capture = new CameraCapture();
    GLOBAL.gdi_capture = new GDICapture();
#endif // WIN32

    blog(LOG_INFO, "CaptureModule: obs startup");
    if (!obs_startup("en-US", nullptr, nullptr))
    {
        return -2;
    }

    blog(LOG_INFO, "CaptureModule: obs reset video");
    if (obs_reset_video(&GLOBAL.video_info) != OBS_VIDEO_SUCCESS)
    {
        return -3;
    }

    blog(LOG_INFO, "CaptureModule: obs reset audio");
    if (!obs_reset_audio(&GLOBAL.audio_info))
    {
        return -4;
    }

    blog(LOG_INFO, "CaptureModule: load all modules");
    // load all modules
    obs_load_all_modules();
    obs_post_load_modules();

    blog(LOG_INFO, "CaptureModule: obs create scene");
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
        return -10;
    }

    video_scale_info video_scale_info;
    video_scale_info.format = VIDEO_FORMAT_NV12;
    video_scale_info.width = GLOBAL.video_info.base_width;
    video_scale_info.height = GLOBAL.video_info.base_height;
    obs_add_raw_video_callback(&video_scale_info, raw_video_callback, nullptr);

    audio_convert_info audio_convert_info;
    audio_convert_info.speakers = SPEAKERS_MONO;
    audio_convert_info.format = AUDIO_FORMAT_16BIT;
    audio_convert_info.samples_per_sec = GLOBAL.audio_info.samples_per_sec;
    obs_add_raw_audio_callback(1, &audio_convert_info, raw_audio_callback, nullptr);

    blog(LOG_INFO, "CaptureModule: capture start done");
    GLOBAL.initialized = true;
    return 0;
}

void capture_stop()
{
    blog(LOG_INFO, "CaptureModule: capture stop");
    std::lock_guard<std::mutex> lock_guard(GLOBAL_MUTEX);

    if (!GLOBAL.initialized)
    {
        return;
    }

    blog(LOG_INFO, "CaptureModule: remove obs output source");
    obs_set_output_source(0, nullptr);
    obs_set_output_source(1, nullptr);

    blog(LOG_INFO, "CaptureModule: remove obs raw callback");
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

#ifdef WIN32
    blog(LOG_INFO, "CaptureModule: camera capture stop");
    GLOBAL.camera_capture->StopCapture();
    delete GLOBAL.camera_capture;
    GLOBAL.camera_capture = nullptr;

    blog(LOG_INFO, "CaptureModule: gdi capture stop");
    GLOBAL.gdi_capture->StopCapture();
    delete GLOBAL.gdi_capture;
    GLOBAL.gdi_capture = nullptr;
#endif

    blog(LOG_INFO, "CaptureModule: obs shutdown");
    obs_shutdown();
    GLOBAL.initialized = false;

    blog(LOG_INFO, "CaptureModule: capture stop done");
}

int capture_set_input(DeviceDescription* description, CaptureSettings* settings)
{
    blog(LOG_INFO, "CaptureModule: capture set input device");
    std::lock_guard<std::mutex> lock_guard(GLOBAL_MUTEX);

    if (
        description->type == DeviceType::kDeviceTypeVideo ||
        (description->type == DeviceType::kDeviceTypeScreen && settings->method == CaptureMethod::GDI))
    {
        blog(LOG_INFO, "CaptureModule: capture gdi or camera, skip obs");

        GLOBAL.allow_obs = false;
        obs_set_output_source(0, nullptr);
        obs_set_output_source(1, nullptr);
    }
    else
    {
        GLOBAL.allow_obs = true;
        obs_set_output_source(0, obs_scene_get_source(GLOBAL.scene));
        obs_set_output_source(1, GLOBAL.audio_source);
    }

    if (description->type == DeviceType::kDeviceTypeVideo)
    {
        blog(LOG_INFO, "CaptureModule: capture camera");

#ifdef WIN32
        return GLOBAL.camera_capture->StartCapture(description->id,
                                                   GLOBAL.video_info.base_width,
                                                   GLOBAL.video_info.base_height,
                                                   GLOBAL.video_info.fps_num,
                                                   [](VideoFrame* frame)
                                                   {
                                                       if (GLOBAL_MUTEX.try_lock())
                                                       {
                                                           if (GLOBAL.output_callback.video != nullptr && GLOBAL.initialized)
                                                           {
                                                               GLOBAL.output_callback.video(GLOBAL.output_callback.ctx, frame);
                                                           }

                                                           GLOBAL_MUTEX.unlock();
                                                       }
                                                   });
#endif
    }
    else if (description->type == DeviceType::kDeviceTypeScreen)
    {
        blog(LOG_INFO, "CaptureModule: capture screen");

        if (settings->method == CaptureMethod::GDI)
        {
            blog(LOG_INFO, "CaptureModule: capture camera, use gdi");
#ifdef WIN32
            return GLOBAL.gdi_capture->StartCapture(description->id,
                                                    GLOBAL.video_info.base_width,
                                                    GLOBAL.video_info.base_height,
                                                    GLOBAL.video_info.fps_num,
                                                    [](VideoFrame* frame)
                                                    {
                                                        if (GLOBAL_MUTEX.try_lock())
                                                        {
                                                            if (GLOBAL.output_callback.video != nullptr && GLOBAL.initialized)
                                                            {
                                                                GLOBAL.output_callback.video(GLOBAL.output_callback.ctx, frame);
                                                            }

                                                            GLOBAL_MUTEX.unlock();
                                                        }
                                                    });
#endif
        }
        else
        {
            update_monitor_settings(description, settings);
        }
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

GetDeviceListResult capture_get_device_list(DeviceType type,
                                            CaptureSettings* settings)
{
    std::lock_guard<std::mutex> lock_guard(GLOBAL_MUTEX);

    DeviceList* list = new DeviceList{};
    list->devices = new DeviceDescription * [100];
    list->size = 0;

    std::string key;
    obs_source_t* source = nullptr;
    if (type == DeviceType::kDeviceTypeVideo)
    {
#ifdef WIN32
        int status = CameraCapture::EnumDevices(list);
        return { status, list };
#endif
    }
    else if (type == DeviceType::kDeviceTypeScreen)
    {
        if (settings->method == CaptureMethod::GDI)
        {
#ifdef WIN32
            int status = GLOBAL.gdi_capture->EnumDevices(list);
            return { status, list };
#endif
        }
        else
        {
            source = GLOBAL.monitor_source;
            key = MONITOR_SOURCE_PROPERTY;
        }
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

                DeviceDescription* device = new DeviceDescription{};
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

void capture_release_device_description(DeviceDescription* description)
{
    delete description;
}

void capture_release_device_list(DeviceList* list)
{
    delete list->devices;
    delete list;
}

void capture_set_logger(Logger logger, void* ctx)
{
    std::lock_guard<std::mutex> lock_guard(GLOBAL_MUTEX);

    GLOBAL.logger = logger;
    GLOBAL.logger_ctx = ctx;
}

void* capture_remove_logger()
{
    std::lock_guard<std::mutex> lock_guard(GLOBAL_MUTEX);

    auto ctx = GLOBAL.logger_ctx;
    GLOBAL.logger_ctx = nullptr;
    GLOBAL.logger = nullptr;
    return ctx;
}
