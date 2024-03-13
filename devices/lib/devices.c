//
//  devices.c
//  devices
//
//  Created by Mr.Panda on 2024/2/14.
//

#include "devices.h"

#ifdef WINDOWS
#define DEVICE "dshow"
#define DEVICE_NAME "dummy"
#elif MACOS
#define DEVICE "avfoundation"
#define DEVICE_NAME ""
#endif

enum AVMediaType kind_into_type(DeviceKind kind)
{
    if (kind == DeviceKindVideo)
    {
        return AVMEDIA_TYPE_VIDEO;
    }
    else
    {
        return AVMEDIA_TYPE_AUDIO;
    }
}

Devices get_devices(DeviceKind kind) {
    Devices devices;
    devices.size = 0;
    devices.items = malloc(sizeof(Device) * 100);
    if (devices.items == NULL)
    {
        return devices;
    }

    AVDeviceInfoList* list = NULL;
    AVFormatContext* ctx = avformat_alloc_context();
    const AVInputFormat* fmt = av_find_input_format(DEVICE);
    if (avdevice_list_input_sources(fmt, DEVICE_NAME, NULL, &list) < 0)
    {
        return devices;
    }

    enum AVMediaType type = kind_into_type(kind);
    for (int i = 0; i < list->nb_devices; i ++) 
    {
        for (int k = 0; k < list->devices[i]->nb_media_types; k ++)
        {
            if (list->devices[i]->media_types[k] == type)
            {
                devices.items[devices.size].kind = kind;
                devices.items[devices.size].name = strdup(list->devices[i]->device_name);
                devices.items[devices.size].description = strdup(list->devices[i]->device_description);
                devices.size ++;
            }
        }
    }

    avdevice_free_list_devices(&list);
    avformat_close_input(&ctx);
    avformat_free_context(ctx);

    return devices;
}

void init()
{
    avdevice_register_all();
}

Devices get_audio_devices() 
{
    return get_devices(DeviceKindAudio);
}

Devices get_video_devices() 
{
    return get_devices(DeviceKindVideo);
}

void release_devices(Devices* devices)
{
    for (size_t i = 0; i < devices->size; i ++)
    {
        free(devices->items[i].description);
        free(devices->items[i].name);
    }

    free(devices->items);
}
