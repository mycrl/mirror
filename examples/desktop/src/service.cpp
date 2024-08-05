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
    _render = new SimpleRender(args,
                               hwnd,
                               hinstance,
                               [&]
                               {
                                   this->Close();
                                   MessageBox(nullptr, TEXT("sender/receiver is closed!"), TEXT("Info"), 0);
                               });
}
#endif

MirrorServiceExt::~MirrorServiceExt()
{
    Close();
}

bool MirrorServiceExt::CreateMirrorSender()
{
    if (_sender != nullptr)
    {
        return true;
    }

    if (!_create_mirror())
    {
        return false;
    }

    SenderOptions options;
    options.video.codec = const_cast<char*>(_args.ArgsParams.encoder.c_str());
    options.video.width = _args.ArgsParams.width;
    options.video.height = _args.ArgsParams.height;
    options.video.frame_rate = _args.ArgsParams.fps;
    options.video.key_frame_interval = 21;
    options.video.bit_rate = 500 * 1024 * 8;
    options.audio.sample_rate = 48000;
    options.audio.bit_rate = 64000;
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

    auto sources = mirror_sender_get_sources(_sender, SourceType::Screen);
    mirror_sender_set_video_source(_sender, &sources.items[0]);

    _render->SetTitle("sender");
    return true;
}

bool MirrorServiceExt::CreateMirrorReceiver()
{
    if (_receiver != nullptr)
    {
        return true;
    }

    if (!_create_mirror())
    {
        return false;
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
    return true;
}

void MirrorServiceExt::Close()
{
    if (!is_created)
    {
        return;
    }

    is_created = false;
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

    if (_mirror != nullptr)
    {
        mirror_destroy(_mirror);
        _mirror = nullptr;
    }

    mirror_shutdown();
    _render->SetTitle("");
    _render->Clear();
}

bool MirrorServiceExt::_create_mirror()
{
    if (is_created)
    {
        return true;
    }

    if (_mirror != nullptr)
    {
        return true;
    }

    is_created = true;
    mirror_startup();

    MirrorOptions mirror_options;
    mirror_options.server = const_cast<char*>(_args.ArgsParams.server.c_str());
    mirror_options.multicast = const_cast<char*>("239.0.0.1");
    mirror_options.mtu = 1400;

    _mirror = mirror_create(mirror_options);
    return _mirror != nullptr;
}
