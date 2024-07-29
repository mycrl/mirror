#include "./service.h"

#ifdef WIN32
MirrorServiceExt::MirrorServiceExt(Args& args, HWND hwnd, HINSTANCE hinstance) 
    : _args(args)
#endif
{
    _options.video.encoder = const_cast<char*>(args.ArgsParams.encoder.c_str());
    _options.video.decoder = const_cast<char*>(args.ArgsParams.decoder.c_str());
    _options.video.width = args.ArgsParams.width;
    _options.video.height = args.ArgsParams.height;
    _options.video.frame_rate = args.ArgsParams.fps;
    _options.video.key_frame_interval = 21;
    _options.video.bit_rate = 500 * 1024 * 8;
    _options.audio.sample_rate = 48000;
    _options.audio.bit_rate = 64000;
    _options.server = const_cast<char*>(args.ArgsParams.server.c_str());
    _options.multicast = const_cast<char*>("239.0.0.1");
    _options.mtu = 1400;

#ifdef WIN32
    _render = new SimpleRender(args,
                               hwnd,
                               hinstance,
                               [&]
                               {
                                   _sender = std::nullopt;
                                   _receiver = std::nullopt;
                                   MessageBox(nullptr, TEXT("sender/receiver is closed!"), TEXT("Info"), 0);
                               });
#endif
}

MirrorServiceExt::~MirrorServiceExt()
{
    if (_mirror)
    {
        delete _mirror;
    }

    delete _render;
}

bool MirrorServiceExt::CreateMirrorSender()
{
    Init(_options);
    _mirror = new MirrorService();

    if (_sender.has_value())
    {
        return true;
    }
    else
    {
        _render->IsRender = false;
    }


    CaptureSettings settings;
    settings.method = CaptureMethod::WGC;
    
    DeviceManagerService::Start();
    auto devices = DeviceManagerService::GetDevices(DeviceKind::Video, &settings);
    if (devices.device_list.size() == 0)
    {
        return false;
    }

    DeviceManagerService::SetInputDevice(devices.device_list[0], &settings);
    _sender = _mirror->CreateSender(_args.ArgsParams.id, _render);
    if (!_sender.has_value())
    {
        return false;
    }

    _sender.value().SetMulticast(true);
    _render->SetTitle("sender");
    return true;
}

bool MirrorServiceExt::CreateMirrorReceiver()
{
    Init(_options);
    _mirror = new MirrorService();

    if (_receiver.has_value())
    {
        return true;
    }
    else
    {
        _render->IsRender = true;
    }

    _receiver = _mirror->CreateReceiver(_args.ArgsParams.id, _render);
    if (!_receiver.has_value())
    {
        return false;
    }

    _render->SetTitle("receiver");
    return true;
}

void MirrorServiceExt::Close()
{
    if (_sender.has_value())
    {
        _sender.value().Close();
        _sender = std::nullopt;
        DeviceManagerService::Stop();
    }

    if (_receiver.has_value())
    {
        _receiver.value().Close();
        _receiver = std::nullopt;
    }

    {
        delete _mirror;
        _mirror = nullptr;
        Quit();
    }

    _render->SetTitle("");
    _render->Clear();
}
