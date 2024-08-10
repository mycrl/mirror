//
//  audio_encoder.cpp
//  codec
//
//  Created by Mr.Panda on 2024/2/14.
//

#include "./codec.h"

extern "C"
{
#include <libavutil/opt.h>
}

AudioEncoder* codec_create_audio_encoder(AudioEncoderSettings* settings)
{
	AudioEncoder* codec = new AudioEncoder{};
	codec->output_packet = new Packet{};

	codec->codec = avcodec_find_encoder_by_name(settings->codec_name);
	if (codec->codec == nullptr)
	{
		codec_release_audio_encoder(codec);
		return nullptr;
	}

	codec->context = avcodec_alloc_context3(codec->codec);
	if (!codec->context)
	{
		codec_release_audio_encoder(codec);
		return nullptr;
	}

    codec->context->thread_count = 4;
	codec->context->thread_type = FF_THREAD_SLICE;
    codec->context->sample_fmt = AV_SAMPLE_FMT_S16;
    codec->context->ch_layout = AV_CHANNEL_LAYOUT_MONO;
    codec->context->flags |= AV_CODEC_FLAG_LOW_DELAY;
	codec->context->flags2 |= AV_CODEC_FLAG2_FAST;

	codec->context->bit_rate = settings->bit_rate;
	codec->context->sample_rate = settings->sample_rate;
	codec->context->time_base = av_make_q(1, settings->sample_rate);

    av_opt_set(codec->context->priv_data, "frame_duration", "100", 0);
	av_opt_set_int(codec->context->priv_data, "application", 2051, 0);
	
	if (avcodec_open2(codec->context, codec->codec, nullptr) != 0)
	{
		codec_release_audio_encoder(codec);
		return nullptr;
	}

	if (avcodec_is_open(codec->context) == 0)
	{
		codec_release_audio_encoder(codec);
		return nullptr;
	}

	codec->packet = av_packet_alloc();
	if (codec->packet == nullptr)
	{
		codec_release_audio_encoder(codec);
		return nullptr;
	}

	codec->frame = av_frame_alloc();
	if (codec->frame == nullptr)
	{
		codec_release_audio_encoder(codec);
		return nullptr;
	}

	return codec;
}

bool codec_audio_encoder_copy_frame(AudioEncoder* codec, AudioFrame* frame)
{
	if (codec->context == nullptr)
	{
		return false;
	}

	codec->frame->nb_samples = frame->frames;
	codec->frame->format = codec->context->sample_fmt;
	codec->frame->ch_layout = codec->context->ch_layout;

	if (av_frame_get_buffer(codec->frame, 0) < 0)
	{
		return false;
	}

	av_samples_fill_arrays(codec->frame->data, 
						   codec->frame->linesize, 
						   (const uint8_t*)frame->data, 
						   1,
						   frame->frames, 
						   AV_SAMPLE_FMT_S16, 
						   0);

	codec->frame->pts = codec->pts;
	codec->pts += codec->context->frame_size;

	return true;
}

bool codec_audio_encoder_send_frame(AudioEncoder* codec)
{
	if (codec->context == nullptr)
	{
		return false;
	}

	if (avcodec_send_frame(codec->context, codec->frame) != 0)
	{
		return false;
	}

	av_frame_unref(codec->frame);
	return true;
}

Packet* codec_audio_encoder_read_packet(AudioEncoder* codec)
{
	if (codec->context == nullptr)
	{
		return nullptr;
	}

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
    codec->output_packet->timestamp = codec->packet->pts;
    
	return codec->output_packet;
}

void codec_unref_audio_encoder_packet(AudioEncoder* codec)
{
	av_packet_unref(codec->packet);
}

void codec_release_audio_encoder(AudioEncoder* codec)
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

	delete codec;
}
