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
    Window,
};

struct VideoOptions
{
    /// Video encoder settings, possible values are `h264_qsv`, `h264_nvenc`,
    /// `libx264` and so on.
	char* encoder;
    /// Video decoder settings, possible values are `h264_qsv`, `h264_cuvid`,
    /// `h264`, etc.
	char* decoder;
    /// Maximum number of B-frames, if low latency encoding is performed, it is
    /// recommended to set it to 0 to indicate that no B-frames are encoded.
	uint8_t max_b_frames;
    /// Frame rate setting in seconds.
	uint8_t frame_rate;
    /// The width of the video.
	uint32_t width;
    /// The height of the video.
	uint32_t height;
    /// The bit rate of the video encoding.
	uint64_t bit_rate;
    /// Keyframe Interval, used to specify how many frames apart to output a
    /// keyframe.
	uint32_t key_frame_interval;
};

struct AudioOptions
{
    /// The sample rate of the audio, in seconds.
    uint64_t sample_rate;
    /// The bit rate of the video encoding.
    uint64_t bit_rate;
};

struct MirrorOptions
{
    /// Video Codec Configuration.
	VideoOptions video;
    /// Audio Codec Configuration.
    AudioOptions audio;
    /// Multicast address, e.g. `239.0.0.1`.
	char* multicast;
    /// The size of the maximum transmission unit of the network, which is
    /// related to the settings of network devices such as routers or switches,
    /// the recommended value is 1400.
	size_t mtu;
};

struct Device
{
	const void* description;
};

struct Devices
{
	/// device list.
	const struct Device* devices;
	/// device vector capacity.
	size_t capacity;
	/// device vector size.
	size_t size;
};

typedef const void* Mirror;
typedef const void* Sender;
typedef const void* Receiver;

struct FrameSink
{
    bool (*video)(void* ctx, struct VideoFrame* frame);
    bool (*audio)(void* ctx, struct AudioFrame* frame);
    void* ctx;
};

extern "C"
{
    /// Automatically search for encoders, limited hardware, fallback to software
    /// implementation if hardware acceleration unit is not found.
    EXPORT const char* mirror_find_video_encoder();
    /// Automatically search for decoders, limited hardware, fallback to software
    /// implementation if hardware acceleration unit is not found.
    EXPORT const char* mirror_find_video_decoder();
    /// Cleans up the environment when the SDK exits, and is recommended to be
    /// called when the application exits.
	EXPORT void mirror_quit();
    /// Initialize the environment, which must be initialized before using the SDK.
	EXPORT bool mirror_init(struct MirrorOptions options);
    /// Get device name.
	EXPORT const char* mirror_get_device_name(const struct Device* device);
    /// Get device kind.
	EXPORT enum DeviceKind mirror_get_device_kind(const struct Device* device);
    /// Get devices from device manager.
	EXPORT struct Devices mirror_get_devices(enum DeviceKind kind);
    /// Release devices.
	EXPORT void mirror_drop_devices(struct Devices* devices);
    /// Setting up an input device, repeated settings for the same type of device
    /// will overwrite the previous device.
	EXPORT bool mirror_set_input_device(const struct Device* device);
    /// Create mirror.
	EXPORT Mirror mirror_create();
    /// Release mirror.
	EXPORT void mirror_drop(Mirror mirror);
    /// Create a sender, specify a bound NIC address, you can pass callback to
    /// get the device screen or sound callback, callback can be null, if it is
    /// null then it means no callback data is needed.
	EXPORT Sender mirror_create_sender(Mirror mirror, char* bind, struct FrameSink sink);
    /// Close sender.
	EXPORT void mirror_close_sender(Sender sender);
    /// Create a receiver, specify a bound NIC address, you can pass callback to
    /// get the sender's screen or sound callback, callback can not be null.
	EXPORT Receiver mirror_create_receiver(Mirror mirror, char* bind, struct FrameSink sink);
    /// Close receiver.
	EXPORT void mirror_close_receiver(Receiver receiver);
}

#ifdef __cplusplus

namespace mirror
{
	class DeviceService
	{
	public:
		DeviceService(struct Device device) : _device(device)
		{}

		std::optional<std::string> GetName()
		{
			auto name = mirror_get_device_name(&_device);
			return name ? std::optional(std::string(name)) : std::nullopt;
		}

		enum DeviceKind GetKind()
		{
			return mirror_get_device_kind(&_device);
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
			mirror_drop_devices(&_devices);
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
			return DeviceList(mirror_get_devices(kind));
		}

		static bool SetInputDevice(DeviceService& device)
		{
			return mirror_set_input_device(device.AsRaw());
		}
	};

	bool Init(struct MirrorOptions options)
	{
		return mirror_init(options);
	}

	void Quit()
	{
		mirror_quit();
	}

	class MirrorService
	{
	public:
		class MirrorSender
		{
		public:
			MirrorSender(Sender sender)
				: _sender(sender)
			{}

			void Close()
			{
				mirror_close_sender(_sender);
			}
		private:
			Sender _sender;
		};

		class MirrorReceiver
		{
		public:
			MirrorReceiver(Receiver receiver)
				: _receiver(receiver)
			{}

			void Close()
			{
				mirror_close_receiver(_receiver);
			}
		private:
			Receiver _receiver;
		};

        class AVFrameSink
        {
        public:
            virtual bool OnVideoFrame(struct VideoFrame* frame) = 0;
            virtual bool OnAudioFrame(struct AudioFrame* frame) = 0;
        };

		MirrorService()
		{
			_mirror = mirror_create();
			if (_mirror == nullptr)
			{
				throw std::runtime_error("Failed to create mirror");
			}
		}

		~MirrorService()
		{
			if (_mirror != nullptr)
			{
				mirror_drop(_mirror);
			}
		}

		std::optional<MirrorSender> CreateSender(std::string& bind, AVFrameSink* sink)
		{
            FrameSink frame_sink;
            frame_sink.video = _video_proc;
            frame_sink.audio = _audio_proc;
            frame_sink.ctx = static_cast<void*>(sink);
			Sender sender = mirror_create_sender(_mirror, const_cast<char*>(bind.c_str()), frame_sink);
			return sender != nullptr ? std::optional(MirrorSender(sender)) : std::nullopt;
		}

		std::optional<MirrorReceiver> CreateReceiver(std::string& bind, AVFrameSink* sink)
		{
			FrameSink frame_sink;
            frame_sink.video = _video_proc;
            frame_sink.audio = _audio_proc;
            frame_sink.ctx = static_cast<void*>(sink);
			Receiver receiver = mirror_create_receiver(_mirror, const_cast<char*>(bind.c_str()), frame_sink);
			return receiver != nullptr ? std::optional(MirrorReceiver(receiver)) : std::nullopt;
		}
	private:
		static bool _video_proc(void* ctx, struct VideoFrame* frame)
		{
			return ((AVFrameSink*)ctx)->OnVideoFrame(frame);
		}

        static bool _audio_proc(void* ctx, struct AudioFrame* frame)
		{
			return ((AVFrameSink*)ctx)->OnAudioFrame(frame);
		}

		Mirror _mirror = nullptr;
	};
}

#endif

#endif /* MIRROR_H */
