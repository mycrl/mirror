//
//  video_encoder.cpp
//  codec
//
//  Created by Mr.Panda on 2024/2/14.
//

#include <string>
#include <libyuv.h>

#include "./codec.h"

#ifdef WIN32
#include <windows.h>
#endif // WIN32

extern "C"
{
#include <libavutil/opt.h>
}

VideoDecoder* codec_create_video_decoder(const char* codec_name)
{
    std::string decoder = std::string(codec_name);
    VideoDecoder* codec = new VideoDecoder{};
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

    codec->context->delay = 0;
    codec->context->max_samples = 1;
    codec->context->has_b_frames = 0;
    codec->context->thread_count = 1;
    codec->context->skip_alpha = true;
    codec->context->pix_fmt = AV_PIX_FMT_NV12;
    codec->context->flags |= AV_CODEC_FLAG_LOW_DELAY;
    codec->context->flags2 |= AV_CODEC_FLAG2_FAST | AV_CODEC_FLAG2_CHUNKS;
    codec->context->hwaccel_flags |= AV_HWACCEL_FLAG_IGNORE_LEVEL | AV_HWACCEL_FLAG_UNSAFE_OUTPUT;

    if (decoder == "h264_qsv")
    {
        av_opt_set_int(codec->context->priv_data, "async_depth", 1, 0);
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

void codec_release_video_decoder(VideoDecoder* codec)
{
    if (codec->format_format.has_value())
    {
        if (codec->format_format.value() != AV_PIX_FMT_NV12)
        {
            for (auto buf : codec->output_frame->data)
            {
                if (buf != nullptr)
                {
                    delete[] buf;
                }
            }
        }
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

bool codec_video_decoder_send_packet(VideoDecoder* codec,
                                     Packet packet)
{
    uint8_t* buf = packet.buffer;
    size_t size = packet.len;

    if (buf == nullptr)
    {
        return true;
    }

    int len;
    while (size)
    {
        /*
        TODO:
        
        After running for a long time, an illegal memory access exception 
        may occur inside this function. This occurs when searching for 
        the h264 nalu start code. 
        There is currently no good way to deal with it, and the cause of 
        the error cannot be found. Therefore, the only way to deal with 
        this illegal memory access exception is to intercept it on 
        Windows and discard the packet.
        */
#ifdef WIN32
        __try {
#endif

            len = av_parser_parse2(codec->parser,
                                   codec->context,
                                   &codec->packet->data,
                                   &codec->packet->size,
                                   buf,
                                   size,
                                   packet.timestamp,
                                   AV_NOPTS_VALUE,
                                   0);
#ifdef WIN32
        }
        __except (EXCEPTION_EXECUTE_HANDLER)
        {
            av_log(nullptr, AV_LOG_ERROR, "av_parser_parse2 EXCEPTION_EXECUTE_HANDLER");
            return true;
        }
#endif
        
        if (len < 0)
		{
			return false;
		}

        buf += len;
        size -= len;

        if (codec->packet->size)
        {
            if (avcodec_send_packet(codec->context, codec->packet) != 0)
            {
                return false;
            }
        }
    }

    return true;
}

VideoFrame* codec_video_decoder_read_frame(VideoDecoder* codec)
{
    av_frame_unref(codec->frame);

    if (avcodec_receive_frame(codec->context, codec->frame) != 0)
    {
        return nullptr;
    }

    codec->output_frame->rect.width = codec->frame->width;
    codec->output_frame->rect.height = codec->frame->height;

    if (codec->frame->format != AV_PIX_FMT_NV12 && !codec->format_format.has_value())
    {
        double size = (double)codec->frame->width * (double)codec->frame->height * 1.5;
        for (int i = 0; i < 2; i++)
        {
            codec->output_frame->data[i] = new uint8_t[(size_t)size];
            codec->output_frame->linesize[i] = codec->frame->width;
        }
    }

    if (!codec->format_format.has_value())
    {
        codec->format_format = std::optional(codec->frame->format);
    }

    if (codec->frame->format != AV_PIX_FMT_NV12)
    {
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
        for (int i = 0; i < 2; i++)
        {
            codec->output_frame->linesize[i] = codec->frame->linesize[i];
            codec->output_frame->data[i] = codec->frame->data[i];
        }
    }

    return codec->output_frame;
}
