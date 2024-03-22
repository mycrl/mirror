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
    devices.items = malloc(sizeof(DeviceInfo*) * 100);
    if (devices.items == NULL)
    {
        return devices;
    }

    const AVInputFormat* fmt = NULL;
    AVDeviceInfoList* list = NULL;
    AVFormatContext* ctx = avformat_alloc_context();

#ifdef WINDOWS
    fmt = av_find_input_format("dshow");
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
                DeviceInfo* device = (DeviceInfo*)malloc(sizeof(DeviceInfo));
                if (device != NULL)
                {
                    device->fmt = fmt;
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

void get_device_info(DeviceInfo* info)
{
    AVDictionary* dict = NULL;
    av_dict_set(&dict, "list_options", "true", 0);

    char name[255] = "";

#ifdef WINDOWS
    strcat(name, info->kind == DeviceKindVideo ? "video=" : "audio=");
    strcat(name, info->name);
#endif

    AVFormatContext* ctx = NULL;
    avformat_open_input(&ctx, name, info->fmt, &dict);
    avformat_close_input(&ctx);
    av_dict_free(&dict);
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

void release_device_info(DeviceInfo* device) 
{
    free(device->description);
    free(device->name);
    free(device);
}

Device* open_device(DeviceInfo* info, DeviceConstraint constraint)
{
    get_device_info(info);

    Device* device = (Device*)malloc(sizeof(Device));
    if (device == NULL)
    {
        return NULL;
    }

    char name[255] = "";
    AVDictionary* options = NULL;

#ifdef WINDOWS
    strcat(name, info->kind == DeviceKindVideo ? "video=" : "audio=");
    strcat(name, info->name);

    char video_size[20] = "";
    sprintf(video_size, "%dx%d", constraint.width, constraint.height);
    av_dict_set(&options, "video_size", video_size, 0);

    char framerate[10] = "";
    sprintf(framerate, "%d", constraint.frame_rate);
    av_dict_set(&options, "framerate", framerate, 0);
#endif

    device->ctx = NULL;
    device->fmt = info->fmt;
    if (avformat_open_input(&device->ctx, name, device->fmt, &options) != 0)
    {
        release_device(device);
        return NULL;
    }

    device->stream_idx = -1;
    for (int i = 0; i < device->ctx->nb_streams; i++)
    {
        if (device->ctx->streams[i]->codecpar->codec_type == kind_into_type(info->kind)) 
        {
            device->stream_idx = i;
            break;
        }
    }

    if (device->stream_idx == -1)
    {
        release_device(device);
        return NULL;
    }

    AVCodecParameters* codec_parameters = device->ctx->streams[device->stream_idx]->codecpar;
    device->codec = avcodec_find_decoder(codec_parameters->codec_id);
    if (!device->codec) 
    {
        release_device(device);
        return NULL;
    }
     
    device->codec_ctx = avcodec_alloc_context3(device->codec);
    if (avcodec_parameters_to_context(device->codec_ctx, codec_parameters) < 0) 
    {
        release_device(device);
        return NULL;
    }
     
    if (avcodec_open2(device->codec_ctx, device->codec, NULL) < 0) 
    {
        release_device(device);
        return NULL;
    }

    device->pkt = av_packet_alloc();
    if (device->pkt == NULL)
    {
        release_device(device);
        return NULL;
    }

    device->frame = av_frame_alloc();
    if (device->frame == NULL)
    {
        release_device(device);
        return NULL;
    }

    device->video_frame = (VideoFrame*)malloc(sizeof(VideoFrame));
    if (device->video_frame == NULL)
    {
        release_device(device);
        return NULL;
    }

    return device;
}

void release_device(Device* device)
{
    if (device->ctx != NULL)
    {
        avformat_close_input(device->ctx);
    }

    if (device->pkt != NULL)
    {
        av_packet_free(device->pkt);
    }

    if (device->video_frame != NULL)
    {
        free(device->video_frame);
    }

    if (device->codec_ctx != NULL)
    {
        avcodec_free_context(device->codec_ctx);
    }

    if (device->frame != NULL)
    {
        av_frame_free(device->frame);
    }

    free(device);
}

int device_advance(Device* device)
{
    av_packet_unref(device->pkt);
    if (av_read_frame(device->ctx, device->pkt) != 0)
    {
        return -1;
    }

    if (device->pkt->stream_index != device->stream_idx)
    {
        return -2;
    }

    avcodec_send_packet(device->codec_ctx, device->pkt);
    return 0;
}

VideoFrame* device_get_frame(Device* device)
{
    if (avcodec_receive_frame(device->codec_ctx, device->frame) != 0)
    {
        return NULL;
    }

    device->video_frame->format = device->frame->format;
    device->video_frame->width = device->frame->width;
    device->video_frame->height = device->frame->height;
    device->video_frame->planes = &device->frame->data;
    device->video_frame->linesizes = &device->frame->linesize;
    return device->video_frame;
}
