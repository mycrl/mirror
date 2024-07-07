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

#include <frame.h>
#include <optional>

extern "C"
{
#include <libavutil/hwcontext.h>
#include <libavcodec/avcodec.h>
#include <libavutil/frame.h>
}

struct Packet
{
	uint8_t* buffer;
	size_t len;
	int flags;
    uint64_t timestamp;
};

struct VideoEncoderSettings
{
	const char* codec_name;
	uint8_t frame_rate;
	uint32_t width;
	uint32_t height;
	uint64_t bit_rate;
	uint32_t key_frame_interval;
};

struct VideoEncoder
{
	bool initialized;
    const AVCodec* codec;
	AVCodecContext* context;
	AVPacket* packet;
	AVFrame* frame;
	Packet* output_packet;
};

struct VideoDecoder
{
    const AVCodec* codec;
	AVCodecContext* context;
	AVCodecParserContext* parser;
	AVPacket* packet;
	AVFrame* frame;
	VideoFrame* output_frame;
	std::optional<int> format_format;
};

struct AudioEncoderSettings
{
	const char* codec_name;
	uint64_t bit_rate;
	uint64_t sample_rate;
};

struct AudioEncoder
{
    const AVCodec* codec;
	AVCodecContext* context;
	AVPacket* packet;
	AVFrame* frame;
	Packet* output_packet;
	uint64_t pts;
};

struct AudioDecoder
{
    const AVCodec* codec;
	AVCodecContext* context;
	AVCodecParserContext* parser;
	AVPacket* packet;
	AVFrame* frame;
	AudioFrame* output_frame;
};

struct CodecDesc
{
	const char* name;
	AVHWDeviceType type;
};

enum CodecKind
{
	Encoder,
	Decoder,
};

typedef void (*Logger)(int level, char* message);

extern "C"
{
	EXPORT void codec_set_logger(Logger logger);
	EXPORT void codec_remove_logger();
	EXPORT const char* codec_find_video_encoder();
	EXPORT const char* codec_find_video_decoder();
	EXPORT VideoEncoder* codec_create_video_encoder(VideoEncoderSettings* settings);
    EXPORT bool codec_video_encoder_copy_frame(VideoEncoder* codec, VideoFrame* frame);
	EXPORT bool codec_video_encoder_send_frame(VideoEncoder* codec);
	EXPORT Packet* codec_video_encoder_read_packet(VideoEncoder* codec);
	EXPORT void codec_unref_video_encoder_packet(VideoEncoder* codec);
	EXPORT void codec_release_video_encoder(VideoEncoder* codec);
	EXPORT VideoDecoder* codec_create_video_decoder(const char* codec_name);
	EXPORT void codec_release_video_decoder(VideoDecoder* codec);
	EXPORT bool codec_video_decoder_send_packet(VideoDecoder* codec, Packet* packet);
	EXPORT VideoFrame* codec_video_decoder_read_frame(VideoDecoder* codec);
	EXPORT AudioEncoder* codec_create_audio_encoder(AudioEncoderSettings* settings);
    EXPORT bool codec_audio_encoder_copy_frame(AudioEncoder* codec, AudioFrame* frame);
	EXPORT bool codec_audio_encoder_send_frame(AudioEncoder* codec);
	EXPORT Packet* codec_audio_encoder_read_packet(AudioEncoder* codec);
	EXPORT void codec_unref_audio_encoder_packet(AudioEncoder* codec);
	EXPORT void codec_release_audio_encoder(AudioEncoder* codec);
	EXPORT AudioDecoder* codec_create_audio_decoder(const char* codec_name);
	EXPORT void codec_release_audio_decoder(AudioDecoder* codec);
	EXPORT bool codec_audio_decoder_send_packet(AudioDecoder* codec, uint8_t* buf, size_t size);
	EXPORT AudioFrame* codec_audio_decoder_read_frame(AudioDecoder* codec);
}

#endif /* codec_h */
