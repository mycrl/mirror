//
//  video_encoder.cpp
//  codec
//
//  Created by Mr.Panda on 2024/2/14.
//

#include "../codec.h"

typedef struct
{
    const char* codec_name;
} VideoEncoderSettings;

typedef struct
{
    
} VideoDecoder;

void create_video_decoder()
{
    VideoDecoder* decoder = new VideoDecoder;
;
	_codec = avcodec_find_decoder_by_name(codec_name.c_str());
	if (!_codec)
	{
		return false;
	}

	_ctx = avcodec_alloc_context3(_codec);
	if (_ctx == nullptr)
	{
		return false;
	}

	if (avcodec_open2(_ctx, _codec, nullptr) != 0)
	{
		return false;
	}

	if (avcodec_is_open(_ctx) == 0)
	{
		return false;
	}

	_parser = av_parser_init(_codec->id);
	if (!_parser)
	{
		return false;
	}

	_packet = av_packet_alloc();
	if (_packet == nullptr)
	{
		return false;
	}

	_frame = av_frame_alloc();
	return _frame != nullptr;
}
