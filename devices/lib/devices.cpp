//
//  devices.c
//  devices
//
//  Created by Mr.Panda on 2024/2/14.
//

#include "devices.h"

static struct
{
    struct obs_video_info video_info;
    obs_scene_t* scene;
    obs_source_t* video_source;
    obs_sceneitem_t* video_scene_item;
    VideoOutputCallback raw_video_callback;
    void* raw_video_callback_context;
    struct VideoFrame video_frame;
} GLOBAL;

void raw_video_callback(void* _, struct video_data *frame)
{
    if (GLOBAL.raw_video_callback == NULL || GLOBAL.raw_video_callback_context == NULL)
    {
        return;
    }

    GLOBAL.video_frame.data[0] = frame->data[0];
    GLOBAL.video_frame.data[1] = frame->data[1];
    GLOBAL.video_frame.linesize[0] = frame->linesize[0];
    GLOBAL.video_frame.linesize[1] = frame->linesize[1];
    GLOBAL.raw_video_callback(GLOBAL.raw_video_callback_context, &GLOBAL.video_frame);
}

void* _set_video_output_callback(VideoOutputCallback proc, void* current_ctx)
{
    void* previous_ctx = GLOBAL.raw_video_callback_context;
    GLOBAL.raw_video_callback_context = current_ctx;
    GLOBAL.raw_video_callback = proc;
    return previous_ctx;
}

int _init(struct VideoInfo* info)
{
    if (obs_initialized())
    {
        return -1;
    }
    
    if (!obs_startup("en-US", NULL, NULL))
    {
        return -2;
    }

    GLOBAL.video_frame.rect.width = info->width;
    GLOBAL.video_frame.rect.height = info->height;
    GLOBAL.video_info.graphics_module = "libobs-d3d11";
    GLOBAL.video_info.fps_num = info->fps;
    GLOBAL.video_info.fps_den = 1;
    GLOBAL.video_info.gpu_conversion = true;
    GLOBAL.video_info.base_width = info->width;
    GLOBAL.video_info.base_height = info->height;
    GLOBAL.video_info.output_width = info->width;
    GLOBAL.video_info.output_height = info->height;
    GLOBAL.video_info.colorspace = VIDEO_CS_DEFAULT;
    GLOBAL.video_info.range = VIDEO_RANGE_DEFAULT;
    GLOBAL.video_info.scale_type = OBS_SCALE_DISABLE;
    GLOBAL.video_info.output_format = VIDEO_FORMAT_NV12;
    GLOBAL.video_info.adapter = 0;
    
    if (obs_reset_video(&GLOBAL.video_info) != OBS_VIDEO_SUCCESS)
    {
        return -3;
    }
    
    obs_load_all_modules();
    obs_post_load_modules();
    obs_add_raw_video_callback(NULL, raw_video_callback, NULL);

    GLOBAL.scene = obs_scene_create("mirror");
    if (GLOBAL.scene == NULL)
    {
        return -4;
    }
    
    GLOBAL.video_source = obs_source_create("dshow_input",
                                            "mirror video input", 
                                            NULL, 
                                            NULL);
    if (GLOBAL.video_source == NULL)
    {
        return -5;
    }
    else
    {
        obs_set_output_source(0, GLOBAL.video_source);
    }
    
    GLOBAL.video_scene_item = obs_scene_add(GLOBAL.scene, GLOBAL.video_source);
    if (GLOBAL.video_scene_item == NULL)
    {
        return -6;
    }
    else
    {
        obs_sceneitem_set_visible(GLOBAL.video_scene_item, true);
    }

    return 0;
}

void _quit()
{
    if (GLOBAL.scene != NULL)
    {
        obs_scene_release(GLOBAL.scene);
    }
    
    if (GLOBAL.video_source != NULL)
    {
        obs_source_release(GLOBAL.video_source);
    }
    
    if (GLOBAL.video_scene_item != NULL)
    {
        obs_sceneitem_release(GLOBAL.video_scene_item);
    }
}

void _set_video_input(struct DeviceDescription* description)
{
    obs_data_t* settings = obs_data_create();
    obs_data_t* cur_settings = obs_source_get_settings(GLOBAL.video_source);
    obs_data_apply(settings, cur_settings);
    
    char resolution[20];
    sprintf(resolution, 
            "%dx%d", 
            GLOBAL.video_info.base_width, 
            GLOBAL.video_info.base_height);
    
    obs_data_set_int(settings, "res_type", 1);
    obs_data_set_bool(settings, "hw_decode", true);
    obs_data_set_string(settings, "resolution", (const char*)&resolution);
    obs_data_set_string(settings, "video_device_id", description->id);
    obs_source_update(GLOBAL.video_source, settings);
    obs_data_release(settings);
}

struct DeviceList _get_device_list(enum DeviceType type)
{
    DeviceList list;
    list.size = 0;
    list.devices = (struct DeviceDescription**)malloc(sizeof(struct DeviceDescription*) * 50);
    
    obs_properties_t* properties = obs_source_properties(GLOBAL.video_source);
    obs_property_t* property = obs_properties_first(properties);
    while (property)
    {
        const char* name = obs_property_name(property);
        if (strcmp(name, "video_device_id") == 0)
        {
            for (size_t i = 0; i < obs_property_list_item_count(property); i++)
            {
                struct DeviceDescription* device = (struct DeviceDescription*)malloc(sizeof(struct DeviceDescription));
                if (device != NULL)
                {
                    device->type = type;
                    device->id = obs_property_list_item_string(property, i);
                    device->name = obs_property_list_item_name(property, i);
                    list.devices[list.size] = device;
                    list.size ++;
                }
            }
        }
        
        obs_property_next(&property);
    }
    
    return list;
}

void _release_device_description(struct DeviceDescription* description)
{
    free(description);
}
