//
//  codec.cpp
//  codec
//
//  Created by Mr.Panda on 2024/2/14.
//

#include <string>

#include "codec.h"

extern "C"
{
#include "libavutil/imgutils.h"
#include "libavutil/opt.h"
}

size_t get_i420_buffer_size(struct VideoFrame& frame, int height)
{
    size_t sizey = frame.stride_y * height;
    size_t sizeu = frame.stride_uv * (height / 2);
    return sizey + (sizeu * 2);
}

struct VideoEncoder* create_video_encoder(struct VideoEncoderSettings* settings)
{
    std::string name = std::string(settings->codec_name);
    struct VideoEncoder* codec = new VideoEncoder();
    
    codec->codec = avcodec_find_encoder_by_name(name.c_str());
    if (!codec->codec)
    {
        delete codec;
        return nullptr;
    }
    
    codec->context = avcodec_alloc_context3(codec->codec);
    if (!codec->context)
    {
        delete codec;
        return nullptr;
    }
    
    codec->context->width = settings->width;
    codec->context->height = settings->height;
    codec->context->bit_rate = settings->bit_rate;
    codec->context->framerate = av_make_q(settings->frame_rate, 1);
    codec->context->time_base = av_make_q(1, settings->frame_rate);
    codec->context->pkt_timebase = av_make_q(1, settings->frame_rate);
    codec->context->gop_size = settings->key_frame_interval;
    codec->context->max_b_frames = settings->max_b_frames;
    codec->context->pix_fmt = AV_PIX_FMT_NV12;
    
    if (name == "h264_qsv")
    {
        av_opt_set_int(codec->context->priv_data, "preset", 7, 0);
        av_opt_set_int(codec->context->priv_data, "profile", 66, 0);
    }
    else if (name == "h264_nvenc")
    {
        av_opt_set_int(codec->context->priv_data, "zerolatency", 1, 0);
        av_opt_set_int(codec->context->priv_data, "b_adapt", 0, 0);
        av_opt_set_int(codec->context->priv_data, "rc", 1, 0);
        av_opt_set_int(codec->context->priv_data, "preset", 3, 0);
        av_opt_set_int(codec->context->priv_data, "profile", 0, 0);
        av_opt_set_int(codec->context->priv_data, "tune", 1, 0);
        av_opt_set_int(codec->context->priv_data, "cq", 30, 0);
    }
    else if (name == "libx264")
    {
        av_opt_set(codec->context->priv_data, "tune", "zerolatency", 0);
    }
    
    if (avcodec_open2(codec->context, codec->codec, nullptr) != 0)
    {
        delete codec;
        return nullptr;
    }

    if (avcodec_is_open(codec->context) == 0)
    {
        delete codec;
        return nullptr;
    }
    
    codec->packet = av_packet_alloc();
    if (codec->packet == nullptr)
    {
        delete codec;
        return nullptr;
    }

    codec->frame = av_frame_alloc();
    if (codec->frame == nullptr)
    {
        delete codec;
        return nullptr;
    }

    codec->frame_num = 0;
    codec->frame->width = codec->context->width;
    codec->frame->height = codec->context->height;
    codec->frame->format = codec->context->pix_fmt;

    if (av_frame_get_buffer(codec->frame, 32) < 0)
    {
        delete codec;
        return nullptr;
    }
    else
    {
        return codec;
    }
}

int video_encoder_send_frame(struct VideoEncoder* codec, struct VideoFrame frame)
{
    if (av_frame_make_writable(codec->frame) != 0)
    {
        return -1;
    }

    int need_size = av_image_fill_arrays(codec->frame->data,
                                         codec->frame->linesize,
                                         frame.buffer,
                                         codec->context->pix_fmt,
                                         codec->context->width,
                                         codec->context->height,
                                         1);
    size_t size = get_i420_buffer_size(frame, codec->context->height);
    if (need_size != size)
    {
        return -1;
    }
    
    if (frame.key_frame)
    {
        codec->frame->key_frame = 1;
    }
    else
    {
        codec->frame->key_frame = 0;
    }
    
    codec->frame->pts = av_rescale_q(codec->frame_num,
                                     codec->context->pkt_timebase,
                                     codec->context->time_base);
    if (avcodec_send_frame(codec->context, codec->frame) != 0)
    {
        return -1;
    }
    else
    {
        codec->frame_num ++;
    }
    
    return 0;
}

struct VideoEncodePacket* video_encoder_read_packet(struct VideoEncoder* codec)
{
    if (avcodec_receive_packet(codec->context, codec->packet) != 0)
    {
        return nullptr;
    }
    
    struct VideoEncodePacket* bytes = new VideoEncodePacket();
    bytes->buffer = codec->packet->data;
    bytes->len = codec->packet->size;
    bytes->flags = codec->packet->flags;
    
    return bytes;
}

void release_video_encode_packet(struct VideoEncoder* codec, struct VideoEncodePacket* packet)
{
    av_packet_unref(codec->packet);
    
    delete packet;
}

void release_video_encoder(struct VideoEncoder* codec)
{
    avcodec_send_frame(codec->context, nullptr);
    avcodec_free_context(&codec->context);
    av_packet_free(&codec->packet);
    av_frame_free(&codec->frame);
    
    delete codec;
}
