#include "./service.h"

#ifdef WIN32
MirrorServiceExt::MirrorServiceExt(Args& args, HWND hwnd, HINSTANCE hinstance) 
    : _args(args)
#endif
{
    MirrorOptions options;
    options.video.encoder = const_cast<char*>(args.ArgsParams.encoder.c_str());
    options.video.decoder = const_cast<char*>(args.ArgsParams.decoder.c_str());
    options.video.width = args.ArgsParams.width;
    options.video.height = args.ArgsParams.height;
    options.video.frame_rate = args.ArgsParams.fps;
    options.video.key_frame_interval = 21;
    options.video.bit_rate = 500 * 1024 * 8;
    options.audio.sample_rate = 48000;
    options.audio.bit_rate = 64000;
    options.server = const_cast<char*>(args.ArgsParams.server.c_str());
    options.multicast = const_cast<char*>("239.0.0.1");
    options.mtu = 1400;
    Init(options);

    _mirror = new MirrorService();
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
    delete _mirror;
    delete _render;
    Quit();
}

bool MirrorServiceExt::CreateMirrorSender()
{
    if (_sender.has_value())
    {
        return true;
    }
    else
    {
        _render->IsRender = false;
    }

    if (_settings.method == CaptureMethod::GDI)
    {
        _settings.method = CaptureMethod::DXGI;
    }
    else if (_settings.method == CaptureMethod::DXGI)
    {
        _settings.method = CaptureMethod::WGC;
    }
    else
    {
        _settings.method = CaptureMethod::GDI;
    }

    DeviceManagerService::Start();
    auto devices = DeviceManagerService::GetDevices(DeviceKind::Screen, &_settings);
    if (devices.device_list.size() == 0)
    {
        return false;
    }

    DeviceManagerService::SetInputDevice(devices.device_list[0], &_settings);
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

    _render->SetTitle("");
    _render->Clear();
}
