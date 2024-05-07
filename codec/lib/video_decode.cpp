//
//  video_encoder.cpp
//  codec
//
//  Created by Mr.Panda on 2024/2/14.
//

#include "./codec.h"

#include <libyuv.h>

struct VideoDecoder* codec_create_video_decoder(const char* codec_name)
{
	struct VideoDecoder* codec = new VideoDecoder{};
	codec->output_frame = new VideoFrame{};

	codec->codec = avcodec_find_decoder_by_name(codec_name);
	if (codec->codec == nullptr)
	{
		codec_release_video_decoder(codec);
		return nullptr;
	}

	codec->context = avcodec_alloc_context3(codec->codec);
	if (codec->context == nullptr)
	{
		codec_release_video_decoder(codec);
		return nullptr;
	}

	if (avcodec_open2(codec->context, codec->codec, nullptr) != 0)
	{
		codec_release_video_decoder(codec);
		return nullptr;
	}

	if (avcodec_is_open(codec->context) == 0)
	{
		codec_release_video_decoder(codec);
		return nullptr;
	}

	codec->parser = av_parser_init(codec->codec->id);
	if (!codec->parser)
	{
		codec_release_video_decoder(codec);
		return nullptr;
	}

	codec->packet = av_packet_alloc();
	if (codec->packet == nullptr)
	{
		codec_release_video_decoder(codec);
		return nullptr;
	}

	codec->frame = av_frame_alloc();
	if (codec->frame == nullptr)
	{
		codec_release_video_decoder(codec);
		return nullptr;
	}

	return codec;
}

void codec_release_video_decoder(struct VideoDecoder* codec)
{
	if (codec->frame->format != AV_PIX_FMT_NV12)
	{
		delete codec->output_frame->data[0];
		delete codec->output_frame->data[1];
	}

	if (codec->context != nullptr)
	{
		avcodec_free_context(&codec->context);
	}

	if (codec->parser != nullptr)
	{
		av_parser_close(codec->parser);
	}

	if (codec->packet != nullptr)
	{
		av_packet_free(&codec->packet);
	}

	if (codec->frame != nullptr)
	{
		av_frame_free(&codec->frame);
	}

	delete codec->output_frame;
	delete codec;
}

bool codec_video_decoder_send_packet(struct VideoDecoder* codec,
									 uint8_t* buf,
									 size_t size)
{
	while (size > 0)
	{
		int ret = av_parser_parse2(codec->parser,
								   codec->context,
								   &codec->packet->data,
								   &codec->packet->size,
								   buf,
								   size,
								   AV_NOPTS_VALUE,
								   AV_NOPTS_VALUE,
								   0);
		if (ret < 0)
		{
			return false;
		}

		buf += ret;
		size -= ret;

		if (codec->packet->size == 0)
		{
			continue;
		}

		if (avcodec_send_packet(codec->context, codec->packet) != 0)
		{
			return false;
		}
	}

	return true;
}

struct VideoFrame* codec_video_decoder_read_frame(struct VideoDecoder* codec)
{
	if (avcodec_receive_frame(codec->context, codec->frame) != 0)
	{
		return nullptr;
	}

	codec->output_frame->rect.width = codec->frame->width;
	codec->output_frame->rect.height = codec->frame->height;

	if (codec->frame->format != AV_PIX_FMT_NV12)
	{
		if (codec->output_frame->data[0] == nullptr)
		{
			size_t size = codec->frame->width * codec->frame->height * 4;
			codec->output_frame->linesize[0] = codec->frame->width;
			codec->output_frame->linesize[1] = codec->frame->width;
			codec->output_frame->data[0] = new uint8_t[size];
			codec->output_frame->data[1] = new uint8_t[size];
		}

		libyuv::I420ToNV12(codec->frame->data[0],
						   codec->frame->linesize[0],
						   codec->frame->data[1],
						   codec->frame->linesize[1],
						   codec->frame->data[2],
						   codec->frame->linesize[2],
						   codec->output_frame->data[0],
						   codec->output_frame->linesize[0],
						   codec->output_frame->data[1],
						   codec->output_frame->linesize[1],
						   codec->frame->width,
						   codec->frame->height);
	}
	else
	{
		codec->output_frame->data[0] = codec->frame->data[0];
		codec->output_frame->data[1] = codec->frame->data[1];
		codec->output_frame->linesize[0] = codec->frame->linesize[0];
		codec->output_frame->linesize[1] = codec->frame->linesize[1];
	}

	return codec->output_frame;
}
