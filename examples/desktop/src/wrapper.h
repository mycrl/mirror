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
    DeviceService(struct Device device);

    std::optional<std::string> GetName();
    enum DeviceKind GetKind();
    struct Device* AsRaw();
private:
    struct Device _device;
};

class DeviceList
{
public:
    DeviceList(struct Devices devices);
    ~DeviceList();

    std::vector<DeviceService> device_list = {};
private:
    struct Devices _devices;
};

class DeviceManagerService
{
public:
    static DeviceList GetDevices(enum DeviceKind kind);
    static bool SetInputDevice(DeviceService& device, CaptureSettings* settings);
    static void Start();
    static void Stop();
};

bool Init(struct MirrorOptions options);
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
    virtual bool OnVideoFrame(struct VideoFrame* frame) = 0;
    virtual bool OnAudioFrame(struct AudioFrame* frame) = 0;
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
    static bool _video_proc(void* ctx, struct VideoFrame* frame);
    static bool _audio_proc(void* ctx, struct AudioFrame* frame);
    static void _close_proc(void* ctx);

    Mirror _mirror = nullptr;
};

#endif