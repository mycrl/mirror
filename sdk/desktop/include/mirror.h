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

struct DeviceManagerOptions
{
	struct DeviceOptions device;
	struct VideoEncoderOptions video_encoder;
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

struct VideoFrame
{
    uint8_t* buffer[4];
    int stride[4];
};

typedef const void* DeviceManager;
typedef const void* Mirror;

typedef bool (*FrameProc)(void* ctx, VideoFrame* frame);

extern "C"
{
	EXPORT DeviceManager create_device_manager(struct DeviceManagerOptions options);
	EXPORT void drop_device_manager(DeviceManager device_manager);
	EXPORT const char* get_device_name(const struct Device* device);
	EXPORT enum DeviceKind get_device_kind(const struct Device* device);
	EXPORT struct Devices get_devices(DeviceManager device_manager);
	EXPORT void drop_devices(struct Devices* devices);
	EXPORT void set_input_device(DeviceManager device_manager, const struct Device* device);
	EXPORT Mirror create_mirror(char* multicast);
	EXPORT void drop_mirror(Mirror mirror);
	EXPORT bool create_sender(Mirror mirror, DeviceManager device_manager, size_t mtu, char* bind);
    EXPORT bool create_receiver(Mirror mirror, char* bind, char* codec, FrameProc proc, void* ctx);
}

#ifdef __cplusplus

namespace mirror
{
	class DeviceManagerService
	{
	public:
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

		DeviceManagerService(struct DeviceManagerOptions options)
		{
			_device_manager = create_device_manager(options);
			if (_device_manager == nullptr)
			{
				throw std::runtime_error("Failed to create mirror");
			}
		}

		~DeviceManagerService()
		{
			if (_device_manager != nullptr)
			{
				drop_device_manager(_device_manager);
			}
		}

		DeviceList GetDevices()
		{
			return DeviceList(get_devices(_device_manager));
		}

		void SetInputDevice(DeviceService& device)
		{
			set_input_device(_device_manager, device.AsRaw());
		}

		DeviceManager AsRaw()
		{
			return _device_manager;
		}
	private:
		DeviceManager _device_manager = nullptr;
	};

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

		bool CreateSender(DeviceManagerService& device_manager, 
						  size_t mtu, 
			              std::string& bind)
		{
			return create_sender(_mirror, 
								 device_manager.AsRaw(), 
				                 mtu, 
				                 const_cast<char*>(bind.c_str()));
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
                            std::string& codec, 
                            FrameProcContext::FrameCallback callback, 
                            void* ctx)
        {
            return create_receiver(_mirror,
                                   const_cast<char*>(bind.c_str()),
                                   const_cast<char*>(codec.c_str()),
                                   _frameProc,
                                   // There is a memory leak, but don't bother caring, 
                                   // it's an infrequently called interface.
                                   new FrameProcContext(callback, ctx));
        }
	private:
        static bool _frameProc(void* ctx, VideoFrame* frame)
        {
            FrameProcContext* context = (FrameProcContext*)ctx;
            context->On(frame);
        }

		Mirror _mirror = nullptr;
	};
}

#endif

#endif /* MIRROR_H */
