#include "./service.h"

bool video_proc(void* ctx, VideoFrame* frame)
{
    auto render = (SimpleRender*)ctx;
    return render->OnVideoFrame(frame);
}

bool audio_proc(void* ctx, AudioFrame* frame)
{
    auto render = (SimpleRender*)ctx;
    return render->OnAudioFrame(frame);
}

void close_proc(void* ctx)
{
    auto render = (SimpleRender*)ctx;
    render->OnClose();
}

#ifdef WIN32
MirrorServiceExt::MirrorServiceExt(Args& args, HWND hwnd, HINSTANCE hinstance)
    : _args(args)
{
    MirrorOptions mirror_options;
    mirror_options.server = const_cast<char*>(_args.ArgsParams.server.c_str());
    mirror_options.multicast = const_cast<char*>("239.0.0.1");
    mirror_options.mtu = 1400;

    _mirror = mirror_create(mirror_options);
    _render = new SimpleRender(args,
                               hwnd,
                               (ID3D11Device*)mirror_get_direct3d_device(_mirror),
                               (ID3D11DeviceContext*)mirror_get_direct3d_device_context(_mirror),
                               [&]
                               {
                                   this->Close();
                                   MessageBox(nullptr, TEXT("sender/receiver is closed!"), TEXT("Info"), 0);
                               });
}
#else
MirrorServiceExt::MirrorServiceExt(Args& args) : _args(args)
{
    _render = new SimpleRender(args,
                               [&]
                               {
                                   this->Close();
                               });
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

    auto video_sources = mirror_get_sources(SourceType::Screen);

    VideoOptions video_options;
    video_options.encoder.codec = const_cast<char*>(_args.ArgsParams.encoder.c_str());
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

    auto audio_sources = mirror_get_sources(SourceType::Audio);

    AudioOptions audio_options;
    audio_options.encoder.sample_rate = 48000;
    audio_options.encoder.bit_rate = 64000;

    for (int i = 0; i < audio_sources.size; i++)
    {
        if (audio_sources.items[i].is_default)
        {
            audio_options.source = &audio_sources.items[i];
        }
    }

    SenderOptions options;
    options.video = &video_options;
    options.audio = &audio_options;
    options.multicast = false;

    FrameSink sink;
    sink.video = video_proc;
    sink.audio = audio_proc;
    sink.close = close_proc;
    sink.ctx = _render;

    _render->IsRender = false;
    _sender = mirror_create_sender(_mirror, _args.ArgsParams.id, options, sink);
    if (_sender == nullptr)
    {
        return false;
    }

    _render->SetTitle("sender");
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
    sink.ctx = _render;

    _render->IsRender = true;
    _receiver = mirror_create_receiver(_mirror,
                                       _args.ArgsParams.id,
                                       _args.ArgsParams.decoder.c_str(),
                                       sink);
    if (_receiver == nullptr)
    {
        return false;
    }

    _render->SetTitle("receiver");
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

    _render->SetTitle("");
    _render->Clear();
}

#ifdef LINUX
void MirrorServiceExt::RunEventLoop(std::function<bool(SDL_Event*)> handler)
{
    _render->RunEventLoop(handler);
}
#endif // LINUX
