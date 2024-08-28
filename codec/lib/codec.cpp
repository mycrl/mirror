//
//  codec.cpp
//  codec
//
//  Created by Mr.Panda on 2024/2/14.
//

#include "./codec.h"

static const char* DefaultVideoDecoder = "h264";
static CodecDesc VideoDecoders[] = {
#ifdef WIN32
    {"d3d11va", AV_HWDEVICE_TYPE_D3D11VA},
#else
    {"h264_qsv", AV_HWDEVICE_TYPE_QSV},
#endif // WIN32
    {"h264_cuvid", AV_HWDEVICE_TYPE_CUDA},
};

static const char* DefaultVideoEncoder = "libx264";
static CodecDesc VideoEncoders[] = {
    {"h264_qsv", AV_HWDEVICE_TYPE_QSV},
    {"h264_nvenc", AV_HWDEVICE_TYPE_CUDA},
};

std::optional<CodecContext> create_video_decoder_context(VideoDecoderSettings* settings)
{
    std::string name = std::string(settings->codec);

    CodecContext ctx;
    ctx.codec = name == "d3d11va" ?
        avcodec_find_decoder(AV_CODEC_ID_H264) :
        avcodec_find_decoder_by_name(name.c_str());
    if (ctx.codec == nullptr)
    {
        return std::nullopt;
    }

    ctx.context = avcodec_alloc_context3(ctx.codec);
    if (ctx.context == nullptr)
    {
        return std::nullopt;
    }

#ifdef WIN32
    if (name == "d3d11va")
    {
        AVBufferRef* hw_device_ctx = av_hwdevice_ctx_alloc(AV_HWDEVICE_TYPE_D3D11VA);
        if (hw_device_ctx == nullptr)
        {
            avcodec_free_context(&ctx.context);
            return std::nullopt;
        }

        AVHWDeviceContext* hwctx = (AVHWDeviceContext*)hw_device_ctx->data;
        AVD3D11VADeviceContext* d3d11_hwctx = (AVD3D11VADeviceContext*)hwctx->hwctx;
        d3d11_hwctx->device_context = settings->d3d11_device_context;
        d3d11_hwctx->device = settings->d3d11_device;

        if (av_hwdevice_ctx_init(hw_device_ctx) != 0)
        {
            avcodec_free_context(&ctx.context);
            return std::nullopt;
        }

        ctx.context->hw_device_ctx = av_buffer_ref(hw_device_ctx);
    }
#endif // WIN32

    return ctx;
}

std::optional<CodecContext> create_video_encoder_context(std::string& name)
{
    CodecContext ctx;
    ctx.codec = avcodec_find_decoder_by_name(name.c_str());
    if (ctx.codec == nullptr)
    {
        return std::nullopt;
    }

    ctx.context = avcodec_alloc_context3(ctx.codec);
    if (ctx.context == nullptr)
    {
        return std::nullopt;
    }

    return ctx;
}

template <size_t S>
const char* find_video_codec(CodecDesc(&codecs)[S], CodecKind kind)
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

/* logger */

static Logger GLOBAL_LOGGER = nullptr;

void logger_proc(void* _, int level, const char* message, va_list args)
{
    if (GLOBAL_LOGGER != nullptr && level <= AV_LOG_VERBOSE)
    {
        char str[8192];
        vsnprintf(str, sizeof(str), message, args);
        GLOBAL_LOGGER(level, str);
    }
}

void codec_set_logger(Logger logger)
{
    if (GLOBAL_LOGGER == nullptr)
    {
        GLOBAL_LOGGER = logger;
        av_log_set_callback(logger_proc);
    }
}

void codec_remove_logger()
{
    GLOBAL_LOGGER = nullptr;
}
