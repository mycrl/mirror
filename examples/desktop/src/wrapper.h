#ifndef WRAPPER_H
#define WRAPPER_H
#pragma once

extern "C"
{
#include <mirror.h>
}

#include <stdexcept>
#include <vector>
#include <string>
#include <optional>
#include <functional>
#include <memory>
#include <tuple>

class DeviceService
{
public:
    DeviceService(Device device);

    std::optional<std::string> GetName();
    DeviceKind GetKind();
    Device* AsRaw();
private:
    Device _device;
};

class DeviceList
{
public:
    DeviceList(Devices devices);
    ~DeviceList();

    std::vector<DeviceService> device_list = {};
private:
    Devices _devices;
};

class DeviceManagerService
{
public:
    static DeviceList GetDevices(DeviceKind kind, CaptureSettings* settings);
    static bool SetInputDevice(DeviceService& device, CaptureSettings* settings);
    static void Start();
    static void Stop();
};

bool Init(MirrorOptions options);
void Quit();

class MirrorSender
{
public:
    MirrorSender(Sender sender);
    void SetMulticast(bool is_multicast);
    bool GetMulticast();
    void Close();
private:
    Sender _sender;
};

class MirrorReceiver
{
public:
    MirrorReceiver(Receiver receiver);
    void Close();
private:
    Receiver _receiver;
};

class AVFrameSink
{
public:
    virtual bool OnVideoFrame(VideoFrame* frame) = 0;
    virtual bool OnAudioFrame(AudioFrame* frame) = 0;
    virtual void OnClose() = 0;
};

class MirrorService
{
public:
    MirrorService();
    ~MirrorService();

    std::optional<MirrorSender> CreateSender(int id, AVFrameSink* sink);
    std::optional<MirrorReceiver> CreateReceiver(int id, AVFrameSink* sink);
private:
    static bool _video_proc(void* ctx, VideoFrame* frame);
    static bool _audio_proc(void* ctx, AudioFrame* frame);
    static void _close_proc(void* ctx);

    Mirror _mirror = nullptr;
};

#endif