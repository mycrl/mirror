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
	Screen,
};

struct VideoOptions
{
	char* encoder;
	char* decoder;
	uint8_t max_b_frames;
	uint8_t frame_rate;
	uint32_t width;
	uint32_t height;
	uint64_t bit_rate;
	uint32_t key_frame_interval;
};

struct MirrorOptions
{
	VideoOptions video;
	char* multicast;
	size_t mtu;
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
typedef const void* Sender;
typedef const void* Receiver;
typedef bool (*ReceiverFrameCallback)(void* ctx, VideoFrame* frame);

extern "C"
{
	EXPORT void quit();
	EXPORT bool init(struct MirrorOptions options);
	EXPORT const char* get_device_name(const struct Device* device);
	EXPORT enum DeviceKind get_device_kind(const struct Device* device);
	EXPORT struct Devices get_devices(enum DeviceKind kind);
	EXPORT void drop_devices(struct Devices* devices);
	EXPORT void set_input_device(const struct Device* device);
	EXPORT Mirror create_mirror();
	EXPORT void drop_mirror(Mirror mirror);
	EXPORT Sender create_sender(Mirror mirror, char* bind, ReceiverFrameCallback proc, void* ctx);
	EXPORT void close_sender(Sender sender);
	EXPORT Receiver create_receiver(Mirror mirror, char* bind, ReceiverFrameCallback proc, void* ctx);
	EXPORT void close_receiver(Receiver receiver);
}

#ifdef __cplusplus

namespace mirror
{
	class DeviceService
	{
	public:
		DeviceService(struct Device device) : _device(device)
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
		DeviceList(struct Devices devices) : _devices(devices)
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
		struct Devices _devices;
	};

	class DeviceManagerService
	{
	public:
		static DeviceList GetDevices(enum DeviceKind kind)
		{
			return DeviceList(get_devices(kind));
		}

		static void SetInputDevice(DeviceService& device)
		{
			set_input_device(device.AsRaw());
		}
	};

	bool Init(struct MirrorOptions options)
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
		class FrameProcContext
		{
		public:
			typedef std::function<bool(void*, struct VideoFrame*)> FrameCallback;

			FrameProcContext(FrameCallback callback, void* ctx)
				: _callback(callback), _ctx(ctx)
			{
			}

			bool On(struct VideoFrame* frame)
			{
				return _callback(_ctx, frame);
			}
		private:
			FrameCallback _callback;
			void* _ctx;
		};

		class MirrorSender
		{
		public:
			MirrorSender(Sender sender, FrameProcContext* ctx)
				: _sender(sender), _ctx(ctx)
			{
			}

			void Close()
			{
				close_sender(_sender);

				if (_ctx != nullptr)
				{
					delete _ctx;
				}
			}
		private:
			Sender _sender;
			FrameProcContext* _ctx;
		};

		class MirrorReceiver
		{
		public:
			MirrorReceiver(Receiver receiver, FrameProcContext* ctx)
				: _receiver(receiver), _ctx(ctx)
			{
			}

			void Close()
			{
				close_receiver(_receiver);
				delete _ctx;
			}
		private:
			Receiver _receiver;
			FrameProcContext* _ctx;
		};

		MirrorService()
		{
			_mirror = create_mirror();
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

		std::optional<MirrorSender> CreateSender(std::string& bind,
			std::optional<FrameProcContext::FrameCallback> callback,
			void* ctx)
		{
			FrameProcContext* ctx_ = callback.has_value() ? new FrameProcContext(callback.value(), ctx) : nullptr;
			Sender sender = create_sender(_mirror, const_cast<char*>(bind.c_str()), callback.has_value() ? _proc : nullptr, ctx_);
			return sender != nullptr ? std::optional(MirrorSender(sender, ctx_)) : std::nullopt;
		}

		std::optional<MirrorReceiver> CreateReceiver(std::string& bind,
			FrameProcContext::FrameCallback callback,
			void* ctx)
		{
			FrameProcContext* ctx_ = new FrameProcContext(callback, ctx);
			Receiver receiver = create_receiver(_mirror, const_cast<char*>(bind.c_str()), _proc, ctx_);
			return receiver != nullptr ? std::optional(MirrorReceiver(receiver, ctx_)) : std::nullopt;
		}
	private:
		static bool _proc(void* ctx, struct VideoFrame* frame)
		{
			return ((FrameProcContext*)ctx)->On(frame);
		}

		Mirror _mirror = nullptr;
	};
}

#endif

#endif /* MIRROR_H */
