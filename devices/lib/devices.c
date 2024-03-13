//
//  devices.c
//  devices
//
//  Created by Mr.Panda on 2024/2/14.
//

#include "devices.h"

void show_dshow_device() {
	AVFormatContext* pFormatCtx = avformat_alloc_context();
	AVDictionary* options = NULL;
	av_dict_set(&options, "list_devices", "true", 0); //0表示不区分大小写
	AVInputFormat* iformat = av_find_input_format("avfoundation");
	printf("========Device Info=============\n");
	avformat_open_input(&pFormatCtx, "", iformat, &options);
	printf("================================\n");
	avformat_free_context(pFormatCtx);
}

void init()
{
    avdevice_register_all();
    show_dshow_device();
}

const AVInputFormat* get_audio_device_next(const AVInputFormat* device) 
{
    return av_input_audio_device_next(device);
}

const AVInputFormat* get_video_device_next(const AVInputFormat* device) 
{
    return av_input_video_device_next(device);
}

const char* get_device_name(const AVInputFormat* device)
{
    return device->long_name;
}
