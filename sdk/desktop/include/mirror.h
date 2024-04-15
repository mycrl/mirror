//
// mirror.h
// mirror
//
// Created by Panda on 2024/4/1.
//

#ifndef MIRROR_H
#define MIRROR_H
#pragma once

#ifdef WIN32
#define EXPORT __declspec(dllexport)
#else
#define EXPORT
#endif

#include <frame.h>

#ifdef __cplusplus

#include <stdexcept>
#include <vector>
#include <string>
#include <optional>
#include <functional>
#include <memory>
#include <tuple>

#endif

enum DeviceKind
{
    Video,
    Audio,
    Screem,
};

struct VideoEncoderOptions
{
    char* codec_name;
    uint8_t max_b_frames;
    uint8_t frame_rate;
    uint32_t width;
    uint32_t height;
    uint64_t bit_rate;
    uint32_t key_frame_interval;
};

struct DeviceOptions
{
    uint8_t fps;
    uint32_t width;
    uint32_t height;
};

struct Device
{
    const void* description;
};

struct Devices
{
    const struct Device* devices;
    size_t capacity;
    size_t size;
};

typedef const void* Mirror;

typedef bool (*FrameProc)(void* ctx, VideoFrame* frame);

extern "C"
{
EXPORT void quit();
EXPORT bool init(struct DeviceOptions options);
EXPORT const char* get_device_name(const struct Device* device);
EXPORT enum DeviceKind get_device_kind(const struct Device* device);
EXPORT struct Devices get_devices(DeviceKind kind);
EXPORT void drop_devices(struct Devices* devices);
EXPORT void set_input_device(const struct Device* device);
EXPORT Mirror create_mirror(char* multicast);
EXPORT void drop_mirror(Mirror mirror);
EXPORT bool create_sender(Mirror mirror, size_t mtu, char* bind, VideoEncoderOptions options);
EXPORT bool create_receiver(Mirror mirror, char* bind, FrameProc proc, void* ctx, char* codec);
}

#ifdef __cplusplus

namespace mirror
{
class DeviceService
{
public:
    DeviceService(struct Device device): _device(device)
    {
    }
    
    std::optional<std::string> GetName()
    {
        auto name = get_device_name(&_device);
        return name ? std::optional(std::string(name)) : std::nullopt;
    }
    
    enum DeviceKind GetKind()
    {
        return get_device_kind(&_device);
    }
    
    struct Device* AsRaw()
    {
        return &_device;
    }
private:
    struct Device _device;
};

class DeviceList
{
public:
    DeviceList(Devices devices): _devices(devices)
    {
        for (size_t i = 0; i < devices.size; i++)
        {
            device_list.push_back(DeviceService(devices.devices[i]));
        }
    }
    
    ~DeviceList()
    {
        drop_devices(&_devices);
    }
    
    std::vector<DeviceService> device_list = {};
private:
    Devices _devices;
};

class DeviceManagerService
{
public:
    static DeviceList GetDevices(DeviceKind kind)
    {
        return DeviceList(get_devices(kind));
    }
    
    static void SetInputDevice(DeviceService& device)
    {
        set_input_device(device.AsRaw());
    }
};

bool Init(struct DeviceOptions options)
{
    return init(options);
}

void Quit()
{
    quit();
}

class MirrorService
{
public:
    MirrorService(std::string multicast)
    {
        _mirror = create_mirror(const_cast<char*>(multicast.c_str()));
        if (_mirror == nullptr)
        {
            throw std::runtime_error("Failed to create mirror");
        }
    }
    
    ~MirrorService()
    {
        if (_mirror != nullptr)
        {
            drop_mirror(_mirror);
        }
    }
    
    bool CreateSender(size_t mtu,
                      std::string& bind,
                      VideoEncoderOptions& options)
    {
        return create_sender(_mirror,
                             mtu,
                             const_cast<char*>(bind.c_str()),
                             options);
    }
    
    class FrameProcContext
    {
    public:
        typedef std::function<bool (void*, VideoFrame*)> FrameCallback;
        
        FrameProcContext(FrameCallback callback, void* ctx)
        : _callback(callback), _ctx(ctx)
        {
        }
        
        bool On(VideoFrame* frame)
        {
            return _callback(_ctx, frame);
        }
    private:
        FrameCallback _callback;
        void* _ctx;
    };
    
    bool CreateReceiver(std::string& bind,
                        FrameProcContext::FrameCallback callback,
                        void* ctx,
                        std::string& codec)
    {
        return create_receiver(_mirror,
                               const_cast<char*>(bind.c_str()),
                               _frameProc,
                               // There is a memory leak, but don't bother caring,
                               // it's an infrequently called interface.
                               new FrameProcContext(callback, ctx),
                               const_cast<char*>(codec.c_str()));
    }
private:
    static bool _frameProc(void* ctx, VideoFrame* frame)
    {
        FrameProcContext* context = (FrameProcContext*)ctx;
        return context->On(frame);
    }
    
    Mirror _mirror = nullptr;
};
}

#endif

#endif /* MIRROR_H */
