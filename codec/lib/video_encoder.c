//
//  video_encoder.c
//  codec
//
//  Created by Mr.Panda on 2024/2/14.
//

#include <stdlib.h>
#include <string.h>

#include "libavutil/imgutils.h"
#include "libavutil/frame.h"
#include "libavutil/opt.h"
#include "codec.h"

#ifndef AV_FRAME_FLAG_KEY
#define AV_FRAME_FLAG_KEY (1 << 1)
#endif

size_t get_i420_buffer_size(VideoFrame* frame, int height)
{
	size_t sizey = frame->stride_y * height;
	size_t sizeu = frame->stride_uv * (height / 2);
	return sizey + (sizeu * 2);
}

VideoEncoder* create_video_encoder(VideoEncoderSettings* settings)
{
	VideoEncoder* codec = (VideoEncoder*)malloc(sizeof(VideoEncoder));
	if (codec == NULL)
	{
		return NULL;
	}

	codec->codec = avcodec_find_encoder_by_name(settings->codec_name);
	if (!codec->codec)
	{
		free(codec);
		return NULL;
	}

	codec->context = avcodec_alloc_context3(codec->codec);
	if (!codec->context)
	{
		free(codec);
		return NULL;
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

	if (strcmp(settings->codec_name, "h264_qsv") == 0)
	{
		av_opt_set_int(codec->context->priv_data, "preset", 7, 0);
		av_opt_set_int(codec->context->priv_data, "profile", 66, 0);
	}
	else if (strcmp(settings->codec_name, "h264_nvenc") == 0)
	{
		av_opt_set_int(codec->context->priv_data, "zerolatency", 1, 0);
		av_opt_set_int(codec->context->priv_data, "b_adapt", 0, 0);
		av_opt_set_int(codec->context->priv_data, "rc", 1, 0);
		av_opt_set_int(codec->context->priv_data, "preset", 3, 0);
		av_opt_set_int(codec->context->priv_data, "profile", 0, 0);
		av_opt_set_int(codec->context->priv_data, "tune", 1, 0);
		av_opt_set_int(codec->context->priv_data, "cq", 30, 0);
	}
	else if (strcmp(settings->codec_name, "libx264") == 0)
	{
		av_opt_set(codec->context->priv_data, "tune", "zerolatency", 0);
	}

	if (avcodec_open2(codec->context, codec->codec, NULL) != 0)
	{
		free(codec);
		return NULL;
	}

	if (avcodec_is_open(codec->context) == 0)
	{
		free(codec);
		return NULL;
	}

	codec->packet = av_packet_alloc();
	if (codec->packet == NULL)
	{
		free(codec);
		return NULL;
	}

	codec->frame = av_frame_alloc();
	if (codec->frame == NULL)
	{
		free(codec);
		return NULL;
	}

	codec->frame_num = 0;
	codec->frame->width = codec->context->width;
	codec->frame->height = codec->context->height;
	codec->frame->format = codec->context->pix_fmt;

	if (av_frame_get_buffer(codec->frame, 32) < 0)
	{
		free(codec);
		return NULL;
	}
	else
	{
		return codec;
	}
}

int video_encoder_send_frame(VideoEncoder* codec, VideoFrame* frame)
{
	if (av_frame_make_writable(codec->frame) != 0)
	{
		return -1;
	}

	int need_size = av_image_fill_arrays(codec->frame->data,
		codec->frame->linesize,
		frame->buffer,
		codec->context->pix_fmt,
		codec->context->width,
		codec->context->height,
		1);
	size_t size = get_i420_buffer_size(frame, codec->context->height);
	if (need_size != size)
	{
		return -1;
	}

	if (frame->key_frame)
	{
		codec->frame->flags = AV_FRAME_FLAG_KEY;
	}
	else
	{
		codec->frame->flags = 0;
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
		codec->frame_num++;
	}

	return 0;
}

VideoEncodePacket* video_encoder_read_packet(VideoEncoder* codec)
{
	if (codec->output_packet == NULL)
	{
		codec->output_packet = (VideoEncodePacket*)malloc(sizeof(VideoEncodePacket));
	}

	if (codec->output_packet == NULL)
	{
		return NULL;
	}

	if (avcodec_receive_packet(codec->context, codec->packet) != 0)
	{
		return NULL;
	}

	codec->output_packet->buffer = codec->packet->data;
	codec->output_packet->len = codec->packet->size;
	codec->output_packet->flags = codec->packet->flags;

	return codec->output_packet;
}

void release_video_encoder_packet(VideoEncoder* codec)
{
	av_packet_unref(codec->packet);
}

void release_video_encoder(VideoEncoder* codec)
{
	avcodec_send_frame(codec->context, NULL);
	avcodec_free_context(&codec->context);
	av_packet_free(&codec->packet);
	av_frame_free(&codec->frame);
	free(codec->output_packet);
	free(codec);
}
