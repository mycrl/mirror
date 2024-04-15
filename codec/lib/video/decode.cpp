//
//  video_encoder.cpp
//  codec
//
//  Created by Mr.Panda on 2024/2/14.
//

#include "../codec.h"

VideoDecoder* _create_video_decoder(const char* codec_name)
{
    VideoDecoder* decoder = new VideoDecoder;
    decoder->output_frame = new VideoFrame;
    
    decoder->codec = avcodec_find_decoder_by_name(codec_name);
    if (decoder->codec == nullptr)
    {
        _release_video_decoder(decoder);
        return nullptr;
    }
    
    decoder->context = avcodec_alloc_context3(decoder->codec);
    if (decoder->context == nullptr)
    {
        _release_video_decoder(decoder);
        return nullptr;
    }
    
    if (avcodec_open2(decoder->context, decoder->codec, nullptr) != 0)
    {
        _release_video_decoder(decoder);
        return nullptr;
    }
    
    if (avcodec_is_open(decoder->context) == 0)
    {
        _release_video_decoder(decoder);
        return nullptr;
    }
    
    decoder->parser = av_parser_init(decoder->codec->id);
    if (!decoder->parser)
    {
        _release_video_decoder(decoder);
        return nullptr;
    }
    
    decoder->packet = av_packet_alloc();
    if (decoder->packet == nullptr)
    {
        _release_video_decoder(decoder);
        return nullptr;
    }
    
    decoder->frame = av_frame_alloc();
    if (decoder->frame == nullptr)
    {
        _release_video_decoder(decoder);
        return nullptr;
    }
    
    return decoder;
}

void _release_video_decoder(VideoDecoder* decoder)
{
    if (decoder->context != nullptr)
    {
        avcodec_free_context(&decoder->context);
    }
    
    if (decoder->parser != nullptr)
    {
        av_parser_close(decoder->parser);
    }
    
    if (decoder->packet != nullptr)
    {
        av_packet_free(&decoder->packet);
    }
    
    if (decoder->frame != nullptr)
    {
        av_frame_free(&decoder->frame);
    }
    
    delete decoder->output_frame;
    delete decoder;
}

bool _video_decoder_send_packet(VideoDecoder* decoder,
                                uint8_t* buf,
                                size_t size)
{
    while (size > 0)
    {
        int ret = av_parser_parse2(decoder->parser,
                                   decoder->context,
                                   &decoder->packet->data,
                                   &decoder->packet->size,
                                   buf,
                                   size,
                                   AV_NOPTS_VALUE,
                                   AV_NOPTS_VALUE,
                                   0);
        if (ret < 0)
        {
            return false;
        }
        
        buf += ret;
        size -= ret;
        
        if (decoder->packet->size == 0)
        {
            continue;
        }
        
        if (avcodec_send_packet(decoder->context, decoder->packet) != 0)
        {
            return false;
        }
    }
    
    return true;
}

VideoFrame* _video_decoder_read_frame(VideoDecoder* decoder)
{
    if (avcodec_receive_frame(decoder->context, decoder->frame) != 0)
    {
        return nullptr;
    }
    
    decoder->output_frame->rect.width = decoder->frame->width;
    decoder->output_frame->rect.height = decoder->frame->height;
    decoder->output_frame->data[0] = decoder->frame->data[0];
    decoder->output_frame->data[1] = decoder->frame->data[1];
    decoder->output_frame->linesize[0] = decoder->frame->linesize[0];
    decoder->output_frame->linesize[1] = decoder->frame->linesize[1];
    return decoder->output_frame;
}
