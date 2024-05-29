//
//  video_encoder.cpp
//  codec
//
//  Created by Mr.Panda on 2024/2/14.
//

#include <string>

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

	codec->context->delay = 0;
	codec->context->max_samples = 1;
	codec->context->has_b_frames = 0;
	codec->context->max_b_frames = 0;
	codec->context->skip_alpha = true;
	codec->context->pix_fmt = AV_PIX_FMT_NV12;
	codec->context->flags = AV_CODEC_FLAG_QSCALE | AV_CODEC_FLAG_LOW_DELAY;
	codec->context->profile = FF_PROFILE_H264_BASELINE;

	codec->context->width = settings->width;
	codec->context->height = settings->height;
	codec->context->bit_rate = settings->bit_rate;
    codec->context->rc_max_rate = settings->bit_rate;
    codec->context->rc_min_rate = settings->bit_rate;
    codec->context->rc_buffer_size = settings->bit_rate;
	codec->context->framerate = av_make_q(settings->frame_rate, 1);
	codec->context->time_base = av_make_q(1, settings->frame_rate);
	codec->context->pkt_timebase = av_make_q(1, settings->frame_rate);
	codec->context->gop_size = settings->key_frame_interval / 2;
	
	auto name = std::string(settings->codec_name);
	if (name == "h264_qsv")
	{
        av_opt_set_int(codec->context->priv_data, "async_depth", 1, 0);
        av_opt_set_int(codec->context->priv_data, "low_power", 1 /* true */, 0);
		av_opt_set_int(codec->context->priv_data, "preset", 7 /* veryfast */, 0);
		av_opt_set_int(codec->context->priv_data, "scenario", 1 /* displayremoting */, 0);
	    av_opt_set_int(codec->context->priv_data, "look_ahead", 0 /* false */, 0);
		av_opt_set_int(codec->context->priv_data, "skip_frame", 3 /* brc_only */, 0);
        av_opt_set_int(codec->context->priv_data, "low_delay_brc", 1 /* true */, 0);
        av_opt_set_int(codec->context->priv_data, "bitrate_limit", 1 /* true */, 0);
        av_opt_set_int(codec->context->priv_data, "max_frame_size", codec->context->rc_max_rate / 8, 0);
        av_opt_set_int(codec->context->priv_data, "max_frame_size_i", codec->context->rc_max_rate / 8, 0);
        av_opt_set_int(codec->context->priv_data, "max_frame_size_p", codec->context->rc_max_rate / 8, 0);
	}
	else if (name == "h264_nvenc")
	{
		av_opt_set_int(codec->context->priv_data, "zerolatency", 1 /* true */, 0);
		av_opt_set_int(codec->context->priv_data, "b_adapt", 0 /* false */, 0);
		av_opt_set_int(codec->context->priv_data, "rc", 2 /* cbr */, 0);
		av_opt_set_int(codec->context->priv_data, "cbr", 1 /* true */, 0);
		av_opt_set_int(codec->context->priv_data, "preset", 7 /* low latency */, 0);
		av_opt_set_int(codec->context->priv_data, "tune", 3 /* ultra low latency */, 0);
	}
	else if (name == "libx264")
	{
		av_opt_set(codec->context->priv_data, "preset", "superfast", 0);
		av_opt_set(codec->context->priv_data, "tune", "zerolatency", 0);
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

	codec->frame->width = codec->context->width;
	codec->frame->height = codec->context->height;
	codec->frame->format = codec->context->pix_fmt;

	int ret = av_frame_get_buffer(codec->frame, 32);
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

	int linesize[4] = { (int)frame->linesize[0], (int)frame->linesize[1], 0, 0 };
#ifdef VERSION_6
	uint8_t* data[4] = { frame->data[0], frame->data[1], nullptr, nullptr };
#else
	const uint8_t* data[4] = { frame->data[0], frame->data[1], nullptr, nullptr };
#endif // VERSION_6

	av_image_copy(codec->frame->data,
				  codec->frame->linesize,
				  data,
				  linesize,
				  codec->context->pix_fmt,
				  codec->context->width,
				  codec->context->height);

#ifdef VERSION_6
	auto count = codec->context->frame_num;
#else
	auto count = codec->context->frame_number;
#endif // VERSION_6

	codec->frame->pts = av_rescale_q(count,
									 codec->context->pkt_timebase,
									 codec->context->time_base);
	if (avcodec_send_frame(codec->context, codec->frame) != 0)
	{
		return false;
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
	codec->output_packet->flags = codec->packet->flags;
	codec->output_packet->len = codec->packet->size;
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
