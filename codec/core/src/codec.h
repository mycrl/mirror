//
//  codec.h
//  codec
//
//  Created by Panda on 2024/2/14.
//

#ifndef codec_h
#define codec_h
#pragma once

#ifdef WIN32
#define EXPORT __declspec(dllexport)
#else
#define EXPORT
#endif

#include <cstdint>

extern "C"
{
#include <libavcodec/avcodec.h>
}

struct VideoEncoderSettings
{
    const char* codec_name;
    uint8_t max_b_frames;
    uint8_t frame_rate;
    uint32_t width;
    uint32_t height;
    uint64_t bit_rate;
    uint32_t key_frame_interval;
};

struct VideoEncoder
{
    const AVCodec* codec;
    AVCodecContext* context;
    AVPacket* packet;
    AVFrame* frame;
    uint64_t frame_num;
};

struct VideoFrame
{
    bool key_frame;
    uint8_t* buffer;
    size_t len;
    uint32_t stride_y;
    uint32_t stride_uv;
};

struct VideoEncodePacket
{
    uint8_t* buffer;
    size_t len;
    int flags;
};

extern "C"
{
EXPORT struct VideoEncoder* create_video_encoder(struct VideoEncoderSettings* settings);
EXPORT int video_encoder_send_frame(struct VideoEncoder* codec, VideoFrame frame);
EXPORT struct VideoEncodePacket* video_encoder_read_packet(struct VideoEncoder* codec);
EXPORT void release_video_encode_packet(struct VideoEncoder* codec);
EXPORT void release_video_encoder(struct VideoEncoder* codec);
}

#endif /* codec_h */
