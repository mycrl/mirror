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

#include <string>
#include <cstddef>

extern "C"
{
    #include <libavcodec/avcodec.h>
}

#include <frame.h>

typedef struct
{
    uint8_t* buffer;
    size_t len;
    int flags;
} VideoEncodePacket;

typedef struct
{
    const char* codec_name;
    uint8_t max_b_frames;
    uint8_t frame_rate;
    uint32_t width;
    uint32_t height;
    uint64_t bit_rate;
    uint32_t key_frame_interval;
} VideoEncoderSettings;

typedef struct
{
    std::string codec_name;
    const AVCodec* codec;
    AVCodecContext* context;
    AVPacket* packet;
    AVFrame* frame;
    uint64_t frame_num;
    VideoEncodePacket* output_packet;
} VideoEncoder;

typedef struct
{
    const AVCodec* codec;
    AVCodecContext* context;
    AVCodecParserContext* parser;
    AVPacket* packet;
    AVFrame* frame;
    VideoFrame* output_frame;
} VideoDecoder;

extern "C"
{
    EXPORT VideoEncoder* _create_video_encoder(VideoEncoderSettings* settings);
    EXPORT bool _video_encoder_send_frame(VideoEncoder* codec, VideoFrame* frame);
    EXPORT VideoEncodePacket* _video_encoder_read_packet(VideoEncoder* codec);
    EXPORT void _unref_video_encoder_packet(VideoEncoder* codec);
    EXPORT void _release_video_encoder(VideoEncoder* codec);
    EXPORT VideoDecoder* _create_video_decoder(const char* codec_name);
    EXPORT void _release_video_decoder(VideoDecoder* decoder);
    EXPORT bool _video_decoder_send_packet(VideoDecoder* decoder, uint8_t* buf, size_t size);
    EXPORT VideoFrame* _video_decoder_read_frame(VideoDecoder* decoder);
}

#endif /* codec_h */
