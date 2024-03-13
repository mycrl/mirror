//
//  devices.c
//  devices
//
//  Created by Mr.Panda on 2024/2/14.
//

#include "devices.h"

void init()
{
    avdevice_register_all();
}

Devices get_audio_devices() 
{
    Devices devices;
    devices.size = 0;
    devices.items = malloc(sizeof(const AVInputFormat*) * 100);
    if (devices.items == NULL)
    {
        return devices;
    }

    const AVInputFormat* item = av_input_audio_device_next(NULL);
    for (;item != NULL; devices.size += 1)
    {
        devices.items[devices.size] = item;
        item = av_input_audio_device_next(item);
    }

    return devices;
}

Devices get_video_devices() 
{
    Devices devices;
    devices.size = 0;
    devices.items = malloc(sizeof(const AVInputFormat*) * 100);
    if (devices.items == NULL)
    {
        return devices;
    }

    const AVInputFormat* item = av_input_video_device_next(NULL);
    for (;item != NULL; devices.size += 1)
    {
        devices.items[devices.size] = item;
        item = av_input_video_device_next(item);
    }

    return devices;
}

void release_devices(Devices* devices)
{
    free(devices->items);
}

const char* get_device_name(const AVInputFormat* device)
{
    return device->long_name;
}
