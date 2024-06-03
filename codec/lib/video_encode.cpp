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
    auto name = std::string(settings->codec_name);

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
    codec->context->flags2 = AV_CODEC_FLAG2_FAST;
	codec->context->flags = AV_CODEC_FLAG_PASS2 | AV_CODEC_FLAG_LOW_DELAY;
	codec->context->profile = FF_PROFILE_H264_BASELINE;

	int bit_rate = settings->bit_rate;
	if (name == "h264_qsv")
	{
		bit_rate = bit_rate / 2;
	}

#ifdef VERSION_6
	codec->context->bit_rate = bit_rate / 2;
#else
	codec->context->bit_rate = bit_rate;
#endif // VERSION_6
    codec->context->rc_max_rate = bit_rate;
    codec->context->rc_buffer_size = bit_rate;
    codec->context->bit_rate_tolerance = bit_rate;
    codec->context->rc_initial_buffer_occupancy = bit_rate * 3 / 4;
	codec->context->framerate = av_make_q(settings->frame_rate, 1);
	codec->context->time_base = av_make_q(1, settings->frame_rate);
	codec->context->pkt_timebase = av_make_q(1, settings->frame_rate);
	codec->context->gop_size = settings->key_frame_interval;
	codec->context->height = settings->height;
	codec->context->width = settings->width;
	
	if (name == "h264_qsv")
	{
        av_opt_set_int(codec->context->priv_data, "async_depth", 1, 0);
        av_opt_set_int(codec->context->priv_data, "forced_idr", 1 /* true */, 0);
        av_opt_set_int(codec->context->priv_data, "low_power", 1 /* true */, 0);
#ifdef VERSION_6
        av_opt_set_int(codec->context->priv_data, "vcm", 1 /* true */, 0);
#else
		av_opt_set_int(codec->context->priv_data, "cavlc", 1 /* true */, 0);
#endif // VERSION_6
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
        av_opt_set_int(codec->context->priv_data, "nal-hrd", 2 /* cbr */, 0);
        av_opt_set_int(codec->context->priv_data, "sc_threshold", settings->key_frame_interval, 0);
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

bool codec_video_encoder_copy_frame(struct VideoEncoder* codec, struct VideoFrame* frame)
{
	if (av_frame_make_writable(codec->frame) != 0)
	{
		return false;
	}

    codec->frame->data[0] = frame->data[0];
    codec->frame->data[1] = frame->data[1];
    codec->frame->linesize[0] = frame->linesize[0];
    codec->frame->linesize[1] = frame->linesize[1];
	return true;
}

bool codec_video_encoder_send_frame(struct VideoEncoder* codec)
{
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
    codec->output_packet->timestamp = codec->packet->pts;
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
