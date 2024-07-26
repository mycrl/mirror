#include "./wrapper.h"

DeviceService::DeviceService(Device device) : _device(device)
{
}

std::optional<std::string> DeviceService::GetName()
{
    auto name = mirror_get_device_name(&_device);
    return name ? std::optional(std::string(name)) : std::nullopt;
}

DeviceKind DeviceService::GetKind()
{
    return mirror_get_device_kind(&_device);
}

Device* DeviceService::AsRaw()
{
    return &_device;
}

DeviceList::DeviceList(Devices devices) : _devices(devices)
{
    for (size_t i = 0; i < devices.size; i++)
    {
        device_list.push_back(DeviceService(devices.devices[i]));
    }
}

DeviceList::~DeviceList()
{
    mirror_devices_destroy(&_devices);
}

DeviceList DeviceManagerService::GetDevices(DeviceKind kind, CaptureSettings* settings)
{
    return DeviceList(mirror_get_devices(kind, settings));
}

bool DeviceManagerService::SetInputDevice(DeviceService& device, CaptureSettings* settings)
{
    return mirror_set_input_device(device.AsRaw(), settings);
}

void DeviceManagerService::Start()
{
    mirror_start_capture();
}

void DeviceManagerService::Stop()
{
    mirror_stop_capture();
}

bool Init(MirrorOptions options)
{
    return mirror_init(options);
}

void Quit()
{
    mirror_quit();
}

MirrorSender::MirrorSender(Sender sender)
    : _sender(sender)
{
}

void MirrorSender::SetMulticast(bool is_multicast)
{
    mirror_sender_set_multicast(_sender, is_multicast);
}

bool MirrorSender::GetMulticast()
{
    return mirror_sender_get_multicast(_sender);
}

void MirrorSender::Close()
{
    if (_sender == nullptr)
    {
        return;
    }

    mirror_sender_destroy(_sender);
    _sender = nullptr;
}

MirrorReceiver::MirrorReceiver(Receiver receiver)
    : _receiver(receiver)
{
}

void MirrorReceiver::Close()
{
    if (_receiver == nullptr)
    {
        return;
    }

    mirror_receiver_destroy(_receiver);
    _receiver = nullptr;
}

MirrorService::MirrorService()
{
    _mirror = mirror_create();
    if (_mirror == nullptr)
    {
        throw std::runtime_error("Failed to create mirror");
    }
}

MirrorService::~MirrorService()
{
    if (_mirror == nullptr)
    {
        return;
    }

    mirror_destroy(_mirror);
    _mirror = nullptr;
}

std::optional<MirrorSender> MirrorService::CreateSender(int id, AVFrameSink* sink)
{
    FrameSink frame_sink;
    frame_sink.video = _video_proc;
    frame_sink.audio = _audio_proc;
    frame_sink.close = _close_proc;
    frame_sink.ctx = static_cast<void*>(sink);
    Sender sender = mirror_create_sender(_mirror, id, frame_sink);
    return sender != nullptr ? std::optional(MirrorSender(sender)) : std::nullopt;
}

std::optional<MirrorReceiver> MirrorService::CreateReceiver(int id, AVFrameSink* sink)
{
    FrameSink frame_sink;
    frame_sink.video = _video_proc;
    frame_sink.audio = _audio_proc;
    frame_sink.close = _close_proc;
    frame_sink.ctx = static_cast<void*>(sink);
    Receiver receiver = mirror_create_receiver(_mirror, id, frame_sink);
    return receiver != nullptr ? std::optional(MirrorReceiver(receiver)) : std::nullopt;
}

bool MirrorService::_video_proc(void* ctx, VideoFrame* frame)
{
    return ((AVFrameSink*)ctx)->OnVideoFrame(frame);
}

bool MirrorService::_audio_proc(void* ctx, AudioFrame* frame)
{
    return ((AVFrameSink*)ctx)->OnAudioFrame(frame);
}

void MirrorService::_close_proc(void* ctx)
{
    ((AVFrameSink*)ctx)->OnClose();
}