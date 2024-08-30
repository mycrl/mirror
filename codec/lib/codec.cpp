//
//  codec.cpp
//  codec
//
//  Created by Mr.Panda on 2024/2/14.
//

#include "./codec.h"

static const char* DefaultVideoDecoder = "h264";
static CodecDesc VideoDecoders[] = {
    {"d3d11va", AV_HWDEVICE_TYPE_D3D11VA},
    {"h264_qsv", AV_HWDEVICE_TYPE_QSV},
    {"h264_cuvid", AV_HWDEVICE_TYPE_CUDA},
};

static const char* DefaultVideoEncoder = "libx264";
static CodecDesc VideoEncoders[] = {
    {"h264_qsv", AV_HWDEVICE_TYPE_QSV},
    {"h264_nvenc", AV_HWDEVICE_TYPE_CUDA},
};

#ifdef WIN32
std::optional<CodecContext> create_video_context(CodecKind kind, 
												 std::string& codec, 
												 int width,
												 int height,
												 ID3D11Device* d3d11_device, 
												 ID3D11DeviceContext* d3d11_device_context)
#else
std::optional<CodecContext> create_video_context(CodecKind kind, std::string& codec)
#endif // WIN32
{
    CodecContext ctx;

    if (kind == CodecKind::Encoder)
    {
        ctx.codec = avcodec_find_encoder_by_name(codec.c_str());
    }
    else
    {
        ctx.codec = codec == "d3d11va" ?
            avcodec_find_decoder(AV_CODEC_ID_H264) :
            avcodec_find_decoder_by_name(codec.c_str());
    }

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
    if (codec == "d3d11va" || codec == "h264_qsv")
    {
        AVBufferRef* hw_device_ctx = av_hwdevice_ctx_alloc(AV_HWDEVICE_TYPE_D3D11VA);
        if (hw_device_ctx == nullptr)
        {
            avcodec_free_context(&ctx.context);
            return std::nullopt;
        }

        if (codec == "h264_qsv")
        {
            ID3D11Multithread* multithread;
            auto ret = d3d11_device->QueryInterface(__uuidof(ID3D11Multithread),
                                                    (void**)&multithread);
            if (FAILED(ret))
            {
                avcodec_free_context(&ctx.context);
                return std::nullopt;
            }

            multithread->SetMultithreadProtected(TRUE);
            multithread->Release();
        }

        AVHWDeviceContext* hwctx = (AVHWDeviceContext*)hw_device_ctx->data;
        AVD3D11VADeviceContext* d3d11_hwctx = (AVD3D11VADeviceContext*)hwctx->hwctx;
        d3d11_hwctx->device_context = d3d11_device_context;
        d3d11_hwctx->device = d3d11_device;

        if (av_hwdevice_ctx_init(hw_device_ctx) != 0)
        {
            avcodec_free_context(&ctx.context);
            return std::nullopt;
        }

        if (codec == "h264_qsv")
        {
            AVBufferRef* qsv_device_ctx = nullptr;
            if (av_hwdevice_ctx_create_derived(&qsv_device_ctx,
                                               AV_HWDEVICE_TYPE_QSV,
                                               hw_device_ctx,
                                               0) != 0)
            {
                avcodec_free_context(&ctx.context);
                return std::nullopt;
            }

            ctx.context->hw_device_ctx = av_buffer_ref(qsv_device_ctx);

            if (kind == CodecKind::Encoder)
            {
                AVBufferRef* hw_frames_ctx = av_hwframe_ctx_alloc(ctx.context->hw_device_ctx);
                if (!hw_frames_ctx)
                {
                    avcodec_free_context(&ctx.context);
                    return std::nullopt;
                }

                AVHWFramesContext* frames_ctx = (AVHWFramesContext*)(hw_frames_ctx->data);
                frames_ctx->sw_format = AV_PIX_FMT_NV12;
                frames_ctx->format = AV_PIX_FMT_QSV;
                frames_ctx->initial_pool_size = 20;
                frames_ctx->width = width;
                frames_ctx->height = height;

                if (av_hwframe_ctx_init(hw_frames_ctx) < 0)
                {
                    avcodec_free_context(&ctx.context);
                    return std::nullopt;
                }

                ctx.context->hw_frames_ctx = av_buffer_ref(hw_frames_ctx);
            }
        }
        else
        {
            ctx.context->hw_device_ctx = av_buffer_ref(hw_device_ctx);
        }
    }
#endif // WIN32

    return ctx;
}

AVFrame* create_video_frame(AVCodecContext* context)
{
    std::string codec_name = std::string(context->codec->name);
    AVFrame* frame = av_frame_alloc();
    if (frame == nullptr)
    {
        return nullptr;
    }

    frame->width = context->width;
    frame->height = context->height;
    frame->format = context->pix_fmt;

    if (codec_name == "h264_qsv")
    {
        if (av_hwframe_get_buffer(context->hw_frames_ctx, frame, 0) < 0)
        {
            av_frame_free(&frame);
            return nullptr;
        }
    }
    else
    {
        if (av_frame_get_buffer(frame, 0) < 0)
        {
            av_frame_free(&frame);
            return nullptr;
        }
    }

    return frame;
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
