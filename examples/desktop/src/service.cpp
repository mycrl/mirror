#include "./service.h"

bool video_proc(void* ctx, VideoFrame* frame)
{
    auto mirror = (MirrorServiceExt*)ctx;
    return mirror->Render->OnVideoFrame(frame);
}

bool audio_proc(void* ctx, AudioFrame* frame)
{
    auto mirror = (MirrorServiceExt*)ctx;
    return mirror->Render->OnAudioFrame(frame);
}

void close_proc(void* ctx)
{
    auto mirror = (MirrorServiceExt*)ctx;
    mirror->Close();
}

#ifdef WIN32
MirrorServiceExt::MirrorServiceExt(Args& args, HWND hwnd)
    : _args(args)
{
    MirrorDescriptor mirror_options;
    mirror_options.server = const_cast<char*>(_args.ArgsParams.server.c_str());
    mirror_options.multicast = const_cast<char*>("239.0.0.1");
    mirror_options.mtu = 1400;

    _mirror = mirror_create(mirror_options);
    Render = new SimpleRender(args, hwnd);
}
#else
MirrorServiceExt::MirrorServiceExt(Args& args,
                                   uint64_t window_handle,
                                   void* display)
    : _args(args)
{
    MirrorDescriptor mirror_options;
    mirror_options.server = const_cast<char*>(_args.ArgsParams.server.c_str());
    mirror_options.multicast = const_cast<char*>("239.0.0.1");
    mirror_options.mtu = 1400;

    _mirror = mirror_create(mirror_options);
    Render = new SimpleRender(args, window_handle, display);
}
#endif

MirrorServiceExt::~MirrorServiceExt()
{
    Close();

    if (_mirror != nullptr)
    {
        mirror_destroy(_mirror);
        _mirror = nullptr;
    }
}

bool MirrorServiceExt::CreateMirrorSender()
{
    if (_sender != nullptr)
    {
        return true;
    }

    auto video_sources = mirror_get_sources(SOURCE_TYPE_CAMERA);

    VideoDescriptor video_options;
    video_options.encoder.codec = _args.ArgsParams.encoder;
    video_options.encoder.width = _args.ArgsParams.width;
    video_options.encoder.height = _args.ArgsParams.height;
    video_options.encoder.frame_rate = _args.ArgsParams.fps;
    video_options.encoder.key_frame_interval = 21;
    video_options.encoder.bit_rate = 500 * 1024 * 8;

    for (int i = 0; i < video_sources.size; i++)
    {
        if (video_sources.items[i].is_default)
        {
            video_options.source = &video_sources.items[i];
        }
    }

    auto audio_sources = mirror_get_sources(SOURCE_TYPE_AUDIO);

    AudioDescriptor audio_options;
    audio_options.encoder.sample_rate = 48000;
    audio_options.encoder.bit_rate = 64000;

    for (int i = 0; i < audio_sources.size; i++)
    {
        if (audio_sources.items[i].is_default)
        {
            audio_options.source = &audio_sources.items[i];
        }
    }

    SenderDescriptor options;
    options.video = &video_options;
    options.audio = nullptr;
    options.multicast = false;

    FrameSink sink;
    sink.video = video_proc;
    sink.audio = audio_proc;
    sink.close = close_proc;
    sink.ctx = this;

    Render->Create();
    Render->SetTitle("sender");
    Render->IsRender = false;

    _sender = mirror_create_sender(_mirror,
                                   _args.ArgsParams.id,
                                   options,
                                   sink);
    if (_sender == nullptr)
    {
        return false;
    }

    _is_runing = true;
    return true;
}

bool MirrorServiceExt::CreateMirrorReceiver()
{
    if (_receiver != nullptr)
    {
        return true;
    }

    FrameSink sink;
    sink.video = video_proc;
    sink.audio = audio_proc;
    sink.close = close_proc;
    sink.ctx = this;

    Render->Create();
    Render->SetTitle("receiver");
    Render->IsRender = true;

    _receiver = mirror_create_receiver(_mirror,
                                       _args.ArgsParams.id,
                                       _args.ArgsParams.decoder,
                                       sink);
    if (_receiver == nullptr)
    {
        return false;
    }

    _is_runing = true;
    return true;
}

void MirrorServiceExt::Close()
{
    if (_is_runing)
    {
        _is_runing = false;
    }
    else
    {
        return;
    }

    if (_sender != nullptr)
    {
        mirror_sender_destroy(_sender);
        _sender = nullptr;
    }

    if (_receiver != nullptr)
    {
        mirror_receiver_destroy(_receiver);
        _receiver = nullptr;
    }

    Render->Close();
}
