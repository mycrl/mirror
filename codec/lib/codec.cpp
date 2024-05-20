//
//  codec.cpp
//  codec
//
//  Created by Mr.Panda on 2024/2/14.
//

#include "codec.h"

static const char* DefaultVideoDecoder = "h264";
static struct CodecDesc VideoDecoders[] = {
	{"h264_qsv", AV_HWDEVICE_TYPE_QSV},
	{"h264_cuvid", AV_HWDEVICE_TYPE_CUDA},
};

static const char* DefaultVideoEncoder = "libx264";
static struct CodecDesc VideoEncoders[] = {
	{"h264_qsv", AV_HWDEVICE_TYPE_QSV},
	{"h264_nvenc", AV_HWDEVICE_TYPE_CUDA},
};

template <size_t S>
const char* find_video_codec(struct CodecDesc(&codecs)[S], enum CodecKind kind)
{
	AVBufferRef* ctx = nullptr;
	for (auto codec : codecs)
	{
		if (av_hwdevice_ctx_create(&ctx, codec.type, nullptr, nullptr, 0) == 0)
		{
			av_buffer_unref(&ctx);
			return codec.name;
		}
	}

	if (ctx != nullptr)
	{
		av_buffer_unref(&ctx);
	}

	return kind == CodecKind::Encoder ? DefaultVideoEncoder : DefaultVideoDecoder;
}

const char* codec_find_video_encoder()
{
	return find_video_codec(VideoEncoders, CodecKind::Encoder);
}

const char* codec_find_video_decoder()
{
	return find_video_codec(VideoDecoders, CodecKind::Decoder);
}
