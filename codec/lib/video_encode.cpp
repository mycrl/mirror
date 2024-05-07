//
//  video_encoder.cpp
//  codec
//
//  Created by Mr.Panda on 2024/2/14.
//

#include "./codec.h"

extern "C"
{
#include <libavutil/imgutils.h>
#include <libavutil/opt.h>
}

struct VideoEncoder* codec_create_video_encoder(struct VideoEncoderSettings* settings)
{
	struct VideoEncoder* codec = new VideoEncoder{};
	codec->output_packet = new EncodePacket{};

	codec->codec = avcodec_find_encoder_by_name(settings->codec_name);
	if (codec->codec == nullptr)
	{
		codec_release_video_encoder(codec);
		return nullptr;
	}

	codec->context = avcodec_alloc_context3(codec->codec);
	if (!codec->context)
	{
		codec_release_video_encoder(codec);
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
	codec->context->max_samples = 1;
	codec->codec_name = std::string(settings->codec_name);

	if (codec->codec_name == "h264_qsv")
	{
		av_opt_set_int(codec->context->priv_data, "preset", 7, 0);
		av_opt_set_int(codec->context->priv_data, "profile", 66, 0);
		av_opt_set_int(codec->context->priv_data, "scenario", 4, 0);
	}
	else if (codec->codec_name == "h264_nvenc")
	{
		av_opt_set_int(codec->context->priv_data, "zerolatency", 1, 0);
		av_opt_set_int(codec->context->priv_data, "b_adapt", 0, 0);
		av_opt_set_int(codec->context->priv_data, "rc", 1, 0);
		av_opt_set_int(codec->context->priv_data, "preset", 3, 0);
		av_opt_set_int(codec->context->priv_data, "profile", 0, 0);
		av_opt_set_int(codec->context->priv_data, "tune", 1, 0);
		av_opt_set_int(codec->context->priv_data, "cq", 30, 0);
	}
	else if (codec->codec_name == "libx264")
	{
		av_opt_set(codec->context->priv_data, "preset", "veryfast", 0);
		av_opt_set(codec->context->priv_data, "tune", "zerolatency", 0);
		av_opt_set(codec->context->priv_data, "profile", "baseline", 0);
	}

	if (avcodec_open2(codec->context, codec->codec, nullptr) != 0)
	{
		codec_release_video_encoder(codec);
		return nullptr;
	}

	if (avcodec_is_open(codec->context) == 0)
	{
		codec_release_video_encoder(codec);
		return nullptr;
	}

	codec->packet = av_packet_alloc();
	if (codec->packet == nullptr)
	{
		codec_release_video_encoder(codec);
		return nullptr;
	}

	codec->frame = av_frame_alloc();
	if (codec->frame == nullptr)
	{
		codec_release_video_encoder(codec);
		return nullptr;
	}

	codec->frame_num = 0;
	codec->frame->width = codec->context->width;
	codec->frame->height = codec->context->height;
	codec->frame->format = codec->context->pix_fmt;

	int ret = av_image_alloc(codec->frame->data,
							 codec->frame->linesize,
							 codec->context->width,
							 codec->context->height,
							 codec->context->pix_fmt,
							 32);
	if (ret < 0)
	{
		codec_release_video_encoder(codec);
		return nullptr;
	}

	return codec;
}

bool codec_video_encoder_send_frame(struct VideoEncoder* codec, struct VideoFrame* frame)
{
	if (av_frame_make_writable(codec->frame) != 0)
	{
		return false;
	}

	uint8_t* data[4] = { frame->data[0], frame->data[1], nullptr, nullptr };
	int linesize[4] = { (int)frame->linesize[0], (int)frame->linesize[1], 0, 0 };

	av_image_copy(codec->frame->data,
				  codec->frame->linesize,
				  data,
				  linesize,
				  codec->context->pix_fmt,
				  codec->context->width,
				  codec->context->height);

	codec->frame->pts = av_rescale_q(codec->frame_num,
									 codec->context->pkt_timebase,
									 codec->context->time_base);
	if (avcodec_send_frame(codec->context, codec->frame) != 0)
	{
		return false;
	}
	else
	{
		codec->frame_num++;
	}

	return true;
}

struct EncodePacket* codec_video_encoder_read_packet(struct VideoEncoder* codec)
{
	if (codec->output_packet == nullptr)
	{
		return nullptr;
	}

	if (avcodec_receive_packet(codec->context, codec->packet) != 0)
	{
		return nullptr;
	}

	codec->output_packet->buffer = codec->packet->data;
	codec->output_packet->len = codec->packet->size;
	codec->output_packet->flags = codec->packet->flags;
	return codec->output_packet;
}

void codec_unref_video_encoder_packet(struct VideoEncoder* codec)
{
	av_packet_unref(codec->packet);
}

void codec_release_video_encoder(struct VideoEncoder* codec)
{
	if (codec->context != nullptr)
	{
		avcodec_free_context(&codec->context);
	}

	if (codec->packet != nullptr)
	{
		av_packet_free(&codec->packet);
	}

	if (codec->frame != nullptr)
	{
		av_frame_free(&codec->frame);
	}

	delete codec->output_packet;
	delete codec;
}
