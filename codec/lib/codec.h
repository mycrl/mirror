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

#include <stdbool.h>
#include <stdint.h>
#include <libavcodec/avcodec.h>

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
	const AVCodec* codec;
	AVCodecContext* context;
	AVPacket* packet;
	AVFrame* frame;
	uint64_t frame_num;
	VideoEncodePacket* output_packet;
} VideoEncoder;

typedef struct
{
	bool key_frame;
	uint8_t* buffer;
	size_t len;
	uint32_t stride_y;
	uint32_t stride_uv;
} VideoFrame;

EXPORT VideoEncoder* create_video_encoder(VideoEncoderSettings* settings);
EXPORT int video_encoder_send_frame(VideoEncoder* codec, VideoFrame* frame);
EXPORT VideoEncodePacket* video_encoder_read_packet(VideoEncoder* codec);
EXPORT void release_video_encoder_packet(VideoEncoder* codec);
EXPORT void release_video_encoder(VideoEncoder* codec);

#endif /* codec_h */
