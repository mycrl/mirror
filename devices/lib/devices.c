//
//  devices.c
//  devices
//
//  Created by Mr.Panda on 2024/2/14.
//

#include "devices.h"

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

#ifdef WINDOWS
    const AVInputFormat* fmt = av_find_input_format("dshow");
    if (avdevice_list_input_sources(fmt, "dummy", NULL, &list) < 0)
    {
        return devices;
    }
#endif

    if (list == NULL)
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
                Device* device = (Device*)malloc(sizeof(Device));
                if (device != NULL)
                {
                    device->kind = kind;
                    device->name = strdup(list->devices[i]->device_name);
                    device->description = strdup(list->devices[i]->device_description);
                    devices.items[devices.size] = device;
                    devices.size ++;
                }
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
    free(devices->items);
}

void release_device(Device* device) 
{
    free(device->description);
    free(device->name);
    free(device);
}

DeviceContext* open_device(Device* device)
{
    DeviceContext* dctx = (DeviceContext*)malloc(sizeof(DeviceContext));
    if (dctx == NULL)
    {
        return NULL;
    }

    dctx->buf = (Buffer*)malloc(sizeof(Buffer));
    if (dctx->buf == NULL)
    {
        release_device_context(dctx);
        return NULL;
    }

#ifdef WINDOWS
    char name[255] = "";
    if (device->kind == DeviceKindVideo)
    {
        strcat(name, "video=");
    }
    else
    {
        strcat(name, "audio=");
    }

    strcat(name, device->name);

    dctx->ctx = NULL;
    dctx->fmt = av_find_input_format("dshow");
    if (avformat_open_input(&dctx->ctx, name, dctx->fmt, NULL) != 0)
    {
        release_device_context(dctx);
        return NULL;
    }
#endif

    dctx->pkt = av_packet_alloc();
    if (dctx->pkt == NULL)
    {
        release_device_context(dctx);
        return NULL;
    }

    return dctx->pkt;
}

void release_device_context(DeviceContext* dctx)
{
    if (dctx->ctx != NULL)
    {
        avformat_close_input(dctx->ctx);
    }

    if (dctx->buf != NULL)
    {
        free(dctx->buf);
    }

    free(dctx);
}

Buffer* device_read_packet(DeviceContext* dctx)
{
    if (av_read_frame(dctx->ctx, dctx->pkt) != 0)
    {
        return NULL;
    }
    else
    {
        av_packet_unref(dctx->pkt);
    }

    dctx->buf->data = dctx->pkt->data;
    dctx->buf->size = dctx->pkt->size;
    return dctx->buf;
}
