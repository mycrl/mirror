//
//  audio_encoder.cpp
//  codec
//
//  Created by Mr.Panda on 2024/2/14.
//

#include "./codec.h"

AudioDecoder* codec_create_audio_decoder(const char* codec_name)
{
	AudioDecoder* codec = new AudioDecoder{};
	codec->output_frame = new AudioFrame{};

	codec->codec = avcodec_find_decoder_by_name(codec_name);
	if (codec->codec == nullptr)
	{
		codec_release_audio_decoder(codec);
		return nullptr;
	}

	codec->context = avcodec_alloc_context3(codec->codec);
	if (codec->context == nullptr)
	{
		codec_release_audio_decoder(codec);
		return nullptr;
	}

	codec->context->thread_count = 1;
	codec->context->request_sample_fmt = AV_SAMPLE_FMT_S16;
	codec->context->ch_layout = AV_CHANNEL_LAYOUT_MONO;
	codec->context->flags |= AV_CODEC_FLAG_LOW_DELAY;
	codec->context->flags2 |= AV_CODEC_FLAG2_FAST;

	if (avcodec_open2(codec->context, codec->codec, nullptr) != 0)
	{
		codec_release_audio_decoder(codec);
		return nullptr;
	}

	if (avcodec_is_open(codec->context) == 0)
	{
		codec_release_audio_decoder(codec);
		return nullptr;
	}

	codec->parser = av_parser_init(codec->codec->id);
	if (!codec->parser)
	{
		codec_release_audio_decoder(codec);
		return nullptr;
	}

	codec->packet = av_packet_alloc();
	if (codec->packet == nullptr)
	{
		codec_release_audio_decoder(codec);
		return nullptr;
	}

	codec->frame = av_frame_alloc();
	if (codec->frame == nullptr)
	{
		codec_release_audio_decoder(codec);
		return nullptr;
	}

	return codec;
}

void codec_release_audio_decoder(AudioDecoder* codec)
{
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

bool codec_audio_decoder_send_packet(AudioDecoder* codec,
									 Packet* packet)
{
	uint8_t* buf = packet->buffer;
	size_t size = packet->len;

    if (buf == nullptr)
    {
        return true;
    }

	while (size > 0)
	{
		int ret = av_parser_parse2(codec->parser,
								   codec->context,
								   &codec->packet->data,
								   &codec->packet->size,
								   buf,
								   size,
								   AV_NOPTS_VALUE,
								   packet->timestamp,
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

AudioFrame* codec_audio_decoder_read_frame(AudioDecoder* codec)
{
	if (avcodec_receive_frame(codec->context, codec->frame) != 0)
	{
		return nullptr;
	}

    codec->output_frame->format = (AudioFormat)codec->frame->format;
	codec->output_frame->frames = codec->frame->nb_samples;
    codec->output_frame->data = codec->frame->data[0];
    
	return codec->output_frame;
}
