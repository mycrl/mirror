//
//  audio_encoder.cpp
//  codec
//
//  Created by Mr.Panda on 2024/2/14.
//

#include "./codec.h"

struct AudioEncoder* codec_create_audio_encoder(struct AudioEncoderSettings* settings)
{
	struct AudioEncoder* codec = new AudioEncoder{};
	codec->output_packet = new EncodePacket{};

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

    codec->context->channels = 1;
    codec->context->sample_fmt = AV_SAMPLE_FMT_S16;
    codec->context->channel_layout = AV_CH_LAYOUT_MONO;
    codec->context->flags = AV_CODEC_FLAG_LOW_DELAY;

	codec->context->bit_rate = settings->bit_rate;
	codec->context->sample_rate = settings->sample_rate;
	
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

	codec->frame->format = codec->context->sample_fmt;
	codec->frame->nb_samples = codec->context->frame_size;
	codec->frame->channel_layout = codec->context->channel_layout;

	if (av_frame_get_buffer(codec->frame, 0) < 0)
	{
		codec_release_audio_encoder(codec);
		return nullptr;
	}

	return codec;
}

bool codec_audio_encoder_copy_frame(struct AudioEncoder* codec, struct AudioFrame* frame)
{
	if (av_frame_make_writable(codec->frame) < 0)
	{
		return false;
	}

	codec->frame->data[0] = frame->data[0];
	codec->frame->data[1] = frame->data[1];
	return true;
}

bool codec_audio_encoder_send_frame(struct AudioEncoder* codec)
{
#ifdef VERSION_6
	auto count = codec->context->frame_num;
#else
	auto count = codec->context->frame_number;
#endif // VERSION_6

	codec->frame->pts = count * codec->context->frame_size;
	if (avcodec_send_frame(codec->context, codec->frame) != 0)
	{
		return false;
	}

	return true;
}

struct EncodePacket* codec_audio_encoder_read_packet(struct AudioEncoder* codec)
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
    codec->output_packet->timestamp = codec->packet->pts;
	return codec->output_packet;
}

void codec_unref_audio_encoder_packet(struct AudioEncoder* codec)
{
	av_packet_unref(codec->packet);
}

void codec_release_audio_encoder(struct AudioEncoder* codec)
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
