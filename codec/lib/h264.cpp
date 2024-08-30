//
//  video_encoder.cpp
//  codec
//
//  Created by Mr.Panda on 2024/2/14.
//

#include <string>
#include <libyuv.h>

#include "./codec.h"

extern "C"
{
#include <libavutil/imgutils.h>
#include <libavutil/opt.h>
}

VideoDecoder* codec_create_video_decoder(VideoDecoderSettings* settings)
{
    VideoDecoder* codec = new VideoDecoder{};
    codec->output_frame = new VideoFrame{};

    std::string codec_name = std::string(settings->codec);
#ifdef WIN32
    auto codec_ctx = create_video_context(CodecKind::Decoder, 
                                          codec_name,
                                          0,
                                          0,
                                          settings->d3d11_device, 
                                          settings->d3d11_device_context);
#else
    auto codec_ctx = create_video_context(CodecKind::Decoder, codec_name);
#endif // WIN32
    if (!codec_ctx.has_value())
    {
        return nullptr;
    }
    else
    {
        codec->context = codec_ctx.value().context;
    }

    codec->context->delay = 0;
    codec->context->max_samples = 1;
    codec->context->has_b_frames = 0;
    codec->context->skip_alpha = true;
    codec->context->flags |= AV_CODEC_FLAG_LOW_DELAY;
    codec->context->flags2 |= AV_CODEC_FLAG2_FAST;
    codec->context->hwaccel_flags |= AV_HWACCEL_FLAG_IGNORE_LEVEL | AV_HWACCEL_FLAG_UNSAFE_OUTPUT;

    if (codec_name == "h264_qsv")
    {
        av_opt_set_int(codec->context->priv_data, "async_depth", 1, 0);
    }

    if (avcodec_open2(codec->context, codec_ctx.value().codec, nullptr) != 0)
    {
        codec_release_video_decoder(codec);
        return nullptr;
    }

    if (avcodec_is_open(codec->context) == 0)
    {
        codec_release_video_decoder(codec);
        return nullptr;
    }

    codec->parser = av_parser_init(codec_ctx.value().codec->id);
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

    return codec;
}

bool codec_video_decoder_send_packet(VideoDecoder* codec, Packet packet)
{
    if (codec->context == nullptr)
    {
        return false;
    }

    uint8_t* buf = packet.buffer;
    size_t size = packet.len;

    if (buf == nullptr)
    {
        return true;
    }

    while (size)
    {
        int len = av_parser_parse2(codec->parser,
                                   codec->context,
                                   &codec->packet->data,
                                   &codec->packet->size,
                                   buf,
                                   size,
                                   packet.timestamp,
                                   AV_NOPTS_VALUE,
                                   0);
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
    if (codec->context == nullptr)
    {
        return nullptr;
    }

    if (codec->frame != nullptr)
    {
        av_frame_free(&codec->frame);
    }

    codec->frame = av_frame_alloc();
    if (codec->frame == nullptr)
    {
        return nullptr;
    }

    if (avcodec_receive_frame(codec->context, codec->frame) != 0)
    {
        return nullptr;
    }

    codec->output_frame->width = codec->frame->width;
    codec->output_frame->height = codec->frame->height;
    codec->output_frame->format = VideoFormat::NV12;

    if (codec->frame->format == AV_PIX_FMT_YUV420P)
    {
        if (codec->output_frame->data[0] == nullptr)
        {
            codec->output_frame->linesize[0] = codec->frame->width;
            codec->output_frame->linesize[1] = codec->frame->width;
            codec->output_frame->hardware = false;

            size_t y_size = (size_t)codec->frame->width * (size_t)codec->frame->height;
            size_t uv_size = (size_t)((float)y_size * 0.5);
            codec->output_frame->data[0] = new uint8_t[y_size + uv_size];
            codec->output_frame->data[1] = (uint8_t*)codec->output_frame->data[0] + y_size;
        }

        libyuv::I420ToNV12(codec->frame->data[0],
                           codec->frame->linesize[0],
                           codec->frame->data[1],
                           codec->frame->linesize[1],
                           codec->frame->data[2],
                           codec->frame->linesize[2],
                           (uint8_t*)codec->output_frame->data[0],
                           codec->output_frame->linesize[0],
                           (uint8_t*)codec->output_frame->data[1],
                           codec->output_frame->linesize[1],
                           codec->frame->width,
                           codec->frame->height);
    }
    else if (codec->frame->format == AV_PIX_FMT_QSV)
    {
        mfxFrameSurface1* surface = (mfxFrameSurface1*)codec->frame->data[3];
        mfxHDLPair* hdl = (mfxHDLPair*)surface->Data.MemId;

        codec->output_frame->data[0] = hdl->first;
        codec->output_frame->data[1] = hdl->second;
        codec->output_frame->hardware = true;
    }
    else if (codec->frame->format == AV_PIX_FMT_D3D11)
    {
        for (int i = 0; i < 2; i++)
        {
            codec->output_frame->data[i] = codec->frame->data[i];
        }

        codec->output_frame->hardware = true;
    }
    else
    {
        for (int i = 0; i < 2; i++)
        {
            codec->output_frame->linesize[i] = codec->frame->linesize[i];
            codec->output_frame->data[i] = codec->frame->data[i];
        }

        codec->output_frame->hardware = false;
    }

    return codec->output_frame;
}

void codec_release_video_decoder(VideoDecoder* codec)
{
    if (codec->context->hw_device_ctx != nullptr)
    {
        av_buffer_unref(&codec->context->hw_device_ctx);
    }

    if (codec->context->hw_frames_ctx != nullptr)
    {
        av_buffer_unref(&codec->context->hw_frames_ctx);
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
        if (codec->frame->format == AV_PIX_FMT_YUV420P)
        {
            if (codec->output_frame->data[0] != nullptr)
            {
                delete[] codec->output_frame->data[0];
            }
        }

        av_frame_free(&codec->frame);
    }

    delete codec->output_frame;
    delete codec;
}

VideoEncoder* codec_create_video_encoder(VideoEncoderSettings* settings)
{
    VideoEncoder* codec = new VideoEncoder{};
    codec->output_packet = new Packet{};

    std::string codec_name = std::string(settings->codec);
#ifdef WIN32
    auto codec_ctx = create_video_context(CodecKind::Encoder,
                                          codec_name,
                                          settings->width,
                                          settings->height,
                                          settings->d3d11_device,
                                          settings->d3d11_device_context);
#else
    auto codec_ctx = create_video_context(CodecKind::Encoder, codec_name);
#endif // WIN32
    if (!codec_ctx.has_value())
    {
        return nullptr;
    }
    else
    {
        codec->context = codec_ctx.value().context;
    }

    codec->context->delay = 0;
    codec->context->max_samples = 1;
    codec->context->has_b_frames = 0;
    codec->context->max_b_frames = 0;
    codec->context->flags2 |= AV_CODEC_FLAG2_FAST;
    codec->context->flags |= AV_CODEC_FLAG_LOW_DELAY | AV_CODEC_FLAG_GLOBAL_HEADER;
    codec->context->profile = FF_PROFILE_H264_BASELINE;

    if (codec_name == "h264_qsv")
    {
        codec->context->pix_fmt = AV_PIX_FMT_QSV;
    }
    else
    {
        codec->context->thread_count = 4;
        codec->context->thread_type = FF_THREAD_SLICE;
        codec->context->pix_fmt = AV_PIX_FMT_NV12;
    }

    int bit_rate = settings->bit_rate;
    if (codec_name == "h264_qsv")
    {
        bit_rate = bit_rate / 2;
    }

    codec->context->bit_rate = bit_rate;
    codec->context->rc_max_rate = bit_rate;
    codec->context->rc_buffer_size = bit_rate;
    codec->context->bit_rate_tolerance = bit_rate;
    codec->context->rc_initial_buffer_occupancy = bit_rate * 3 / 4;
    codec->context->framerate = av_make_q(settings->frame_rate, 1);
    codec->context->time_base = av_make_q(1, settings->frame_rate);
    codec->context->pkt_timebase = av_make_q(1, settings->frame_rate);
    codec->context->gop_size = settings->key_frame_interval / 2;
    codec->context->height = settings->height;
    codec->context->width = settings->width;

    if (codec_name == "h264_qsv")
    {
        av_opt_set_int(codec->context->priv_data, "async_depth", 1, 0);
        av_opt_set_int(codec->context->priv_data, "low_power", 1 /* true */, 0);
        av_opt_set_int(codec->context->priv_data, "vcm", 1 /* true */, 0);
    }
    else if (codec_name == "h264_nvenc")
    {
        av_opt_set_int(codec->context->priv_data, "zerolatency", 1 /* true */, 0);
        av_opt_set_int(codec->context->priv_data, "b_adapt", 0 /* false */, 0);
        av_opt_set_int(codec->context->priv_data, "rc", 2 /* cbr */, 0);
        av_opt_set_int(codec->context->priv_data, "cbr", 1 /* true */, 0);
        av_opt_set_int(codec->context->priv_data, "preset", 7 /* low latency */, 0);
        av_opt_set_int(codec->context->priv_data, "tune", 3 /* ultra low latency */, 0);
    }
    else if (codec_name == "libx264")
    {
        av_opt_set(codec->context->priv_data, "preset", "superfast", 0);
        av_opt_set(codec->context->priv_data, "tune", "zerolatency", 0);
        av_opt_set_int(codec->context->priv_data, "nal-hrd", 2 /* cbr */, 0);
        av_opt_set_int(codec->context->priv_data, "sc_threshold", settings->key_frame_interval, 0);
    }

    if (avcodec_open2(codec->context, codec_ctx.value().codec, nullptr) != 0)
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

    codec->frame = create_video_frame(codec->context);
    if (codec->frame == nullptr)
    {
        codec_release_video_encoder(codec);
        return nullptr;
    }

    return codec;
}

bool codec_video_encoder_copy_frame(VideoEncoder* codec, VideoFrame* frame)
{
    if (codec->context == nullptr)
    {
        return false;
    }

    if (frame->hardware)
    {
        if (codec->frame->format == AV_PIX_FMT_QSV)
        {
            mfxFrameSurface1* surface = (mfxFrameSurface1*)codec->frame->data[3];
            mfxHDLPair* hdl = (mfxHDLPair*)surface->Data.MemId;

            hdl->first = frame->data[0];
            hdl->second = frame->data[1];
        }
    }
    else
    {
        if (av_frame_make_writable(codec->frame) != 0)
        {
            return false;
        }

        const uint8_t* buffer[2] =
        {
            (uint8_t*)frame->data[0],
            (uint8_t*)frame->data[1],
        };

        const int linesize[2] =
        {
            (int)frame->linesize[0],
            (int)frame->linesize[1],
        };

        av_image_copy(codec->frame->data,
                      codec->frame->linesize,
                      buffer,
                      linesize,
                      codec->context->pix_fmt,
                      codec->frame->width,
                      codec->frame->height);
    }
    
    return true;
}

bool codec_video_encoder_send_frame(VideoEncoder* codec)
{
    if (codec->context == nullptr)
    {
        return false;
    }

    codec->frame->pts = av_rescale_q(codec->context->frame_num,
                                     codec->context->pkt_timebase,
                                     codec->context->time_base);
    if (avcodec_send_frame(codec->context, codec->frame) != 0)
    {
        return false;
    }

    return true;
}

Packet* codec_video_encoder_read_packet(VideoEncoder* codec)
{
    if (codec->context == nullptr)
    {
        return nullptr;
    }

    if (codec->output_packet == nullptr)
    {
        return nullptr;
    }

    if (!codec->initialized)
    {
        codec->initialized = true;
        codec->output_packet->flags = 2; // BufferFlag::Config
        codec->output_packet->buffer = codec->context->extradata;
        codec->output_packet->len = codec->context->extradata_size;
        codec->output_packet->timestamp = codec->packet->pts;

        return codec->output_packet;
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

void codec_unref_video_encoder_packet(VideoEncoder* codec)
{
    av_packet_unref(codec->packet);
}

void codec_release_video_encoder(VideoEncoder* codec)
{
    if (codec->context->hw_device_ctx != nullptr)
    {
        av_buffer_unref(&codec->context->hw_device_ctx);
    }

    if (codec->context->hw_frames_ctx != nullptr)
    {
        av_buffer_unref(&codec->context->hw_frames_ctx);
    }

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
