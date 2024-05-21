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

#include <optional>
#include <frame.h>

extern "C"
{
#include <libavutil/hwcontext.h>
#include <libavcodec/avcodec.h>
#include <libavutil/frame.h>
}

struct EncodePacket
{
	uint8_t* buffer;
	size_t len;
	int flags;
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
#ifdef VERSION_6
	const AVCodec* codec;
#else
	AVCodec* codec;
#endif // VERSION_6
	AVCodecContext* context;
	AVPacket* packet;
	AVFrame* frame;
	struct EncodePacket* output_packet;
};

struct VideoDecoder
{
#ifdef VERSION_6
	const AVCodec* codec;
#else
	AVCodec* codec;
#endif // VERSION_6
	AVCodecContext* context;
	AVCodecParserContext* parser;
	AVPacket* packet;
	AVFrame* frame;
	struct VideoFrame* output_frame;
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
#ifdef VERSION_6
	const AVCodec* codec;
#else
	AVCodec* codec;
#endif // VERSION_6
	AVCodecContext* context;
	AVPacket* packet;
	AVFrame* frame;
	struct EncodePacket* output_packet;
};

struct AudioDecoder
{
#ifdef VERSION_6
	const AVCodec* codec;
#else
	AVCodec* codec;
#endif // VERSION_6
	AVCodecContext* context;
	AVCodecParserContext* parser;
	AVPacket* packet;
	AVFrame* frame;
	struct AudioFrame* output_frame;
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

extern "C"
{
	EXPORT const char* codec_find_video_encoder();
	EXPORT const char* codec_find_video_decoder();
	EXPORT struct VideoEncoder* codec_create_video_encoder(struct VideoEncoderSettings* settings);
	EXPORT bool codec_video_encoder_send_frame(struct VideoEncoder* codec, struct VideoFrame* frame);
	EXPORT struct EncodePacket* codec_video_encoder_read_packet(struct VideoEncoder* codec);
	EXPORT void codec_unref_video_encoder_packet(struct VideoEncoder* codec);
	EXPORT void codec_release_video_encoder(struct VideoEncoder* codec);
	EXPORT struct VideoDecoder* codec_create_video_decoder(const char* codec_name);
	EXPORT void codec_release_video_decoder(struct VideoDecoder* codec);
	EXPORT bool codec_video_decoder_send_packet(struct VideoDecoder* codec, uint8_t* buf, size_t size);
	EXPORT struct VideoFrame* codec_video_decoder_read_frame(struct VideoDecoder* codec);
	EXPORT struct AudioEncoder* codec_create_audio_encoder(struct AudioEncoderSettings* settings);
	EXPORT bool codec_audio_encoder_send_frame(struct AudioEncoder* codec, struct AudioFrame* frame);
	EXPORT struct EncodePacket* codec_audio_encoder_read_packet(struct AudioEncoder* codec);
	EXPORT void codec_unref_audio_encoder_packet(struct AudioEncoder* codec);
	EXPORT void codec_release_audio_encoder(struct AudioEncoder* codec);
	EXPORT struct AudioDecoder* codec_create_audio_decoder(const char* codec_name);
	EXPORT void codec_release_audio_decoder(struct AudioDecoder* codec);
	EXPORT bool codec_audio_decoder_send_packet(struct AudioDecoder* codec, uint8_t* buf, size_t size);
	EXPORT struct AudioFrame* codec_audio_decoder_read_frame(struct AudioDecoder* codec);
}

#endif /* codec_h */
