#include "./capture.h"

std::string kind_into_string(DeviceKind kind)
{
    if (kind == DeviceKind::Audio)
    {
        return "audio";
    }
    else if (kind == DeviceKind::Video)
    {
        return "video";
    }
    else if (kind == DeviceKind::Screen)
    {
        return "screen";
    }
    else
    {
        return "window";
    }
}

std::optional<DeviceKind> kind_from_string(std::string& kind)
{
    if (kind == "audio")
    {
        return std::optional(DeviceKind::Audio);
    }
    else if (kind == "video")
    {
        return std::optional(DeviceKind::Video);
    }
    else if (kind == "screen")
    {
        return std::optional(DeviceKind::Screen);
    }
    else if (kind == "window")
    {
        return std::optional(DeviceKind::Window);
    }
    else
    {
        return std::nullopt;
    }
}

void CaptureService::Create(Napi::Env env, Napi::Object exports)
{
    auto props =
    {
        InstanceMethod<&CaptureService::StartCapture>("start"),
        InstanceMethod<&CaptureService::StopCapture>("stop"),
        InstanceMethod<&CaptureService::GetDevices>("get_devices"),
        InstanceMethod<&CaptureService::SetInputDevice>("set_input_device"),
    };

    exports.Set("CaptureService", DefineClass(env, "CaptureService", props));
}

CaptureService::CaptureService(const Napi::CallbackInfo& info) : Napi::ObjectWrap<CaptureService>(info)
{
}

Napi::Value CaptureService::StartCapture(const Napi::CallbackInfo& info)
{
    auto env = info.Env();
    return Napi::Boolean::New(env, mirror_start_capture() == 0);
}

Napi::Value CaptureService::StopCapture(const Napi::CallbackInfo& info)
{
    if (_devices.has_value())
    {
        mirror_devices_destroy(&_devices.value());
        _devices = std::nullopt;
    }

    auto env = info.Env();
    mirror_stop_capture();
    return env.Null();

}

Napi::Object create_device_object(Napi::Env& env, int index, const Device* device)
{
    auto id = mirror_get_device_name(device);
    auto kind = kind_into_string(mirror_get_device_kind(device));

    Napi::Object object = Napi::Object::New(env);
    object.Set("id", Napi::String::From(env, id));
    object.Set("kind", Napi::String::From(env, kind));
    object.Set("index", Napi::Number::From(env, index));
    return object;
}

Napi::Value CaptureService::GetDevices(const Napi::CallbackInfo& info)
{
    auto env = info.Env();
    if (info.Length() != 1 || !info[0].IsString())
    {
        Napi::TypeError::New(env, "invalid arguments").ThrowAsJavaScriptException();
        return env.Null();
    }

    std::string kind_str = info[0].As<Napi::String>();
    auto kind = kind_from_string(kind_str);
    if (!kind.has_value())
    {
        Napi::TypeError::New(env, "invalid arguments").ThrowAsJavaScriptException();
        return env.Null();
    }

    CaptureSettings settings;
    settings.method = CaptureMethod::WGC;
    auto devices = mirror_get_devices(kind.value(), &settings);

    Napi::Array list = Napi::Array::New(env);
    for (int i = 0; i < devices.size; i++)
    {
        list.Set(i, create_device_object(env, i, &devices.devices[i]));
    }

    _devices = std::optional(devices);
    list.AddFinalizer(_devices_finalizer, this);

    return list;
}

Napi::Value CaptureService::SetInputDevice(const Napi::CallbackInfo& info)
{
    auto env = info.Env();
    if (info.Length() != 1 || !info[0].IsObject())
    {
        Napi::TypeError::New(env, "invalid arguments").ThrowAsJavaScriptException();
        return env.Null();
    }

    if (!_devices.has_value())
    {
        Napi::Error::New(env, "devices is empty").ThrowAsJavaScriptException();
        return env.Null();
    }

    auto devices = _devices.value();
    auto object = info[0].As<Napi::Object>();
    auto index = object.Get("index").Unwrap().As<Napi::Number>().Uint32Value();
    if (index >= devices.size)
    {
        Napi::Error::New(env, "device not found").ThrowAsJavaScriptException();
        return env.Null();
    }

    CaptureSettings settings;
    settings.method = CaptureMethod::WGC;
    if (!mirror_set_input_device(&devices.devices[index], &settings))
    {
        Napi::Error::New(env, "failed to set device").ThrowAsJavaScriptException();
        return env.Null();
    }

    return env.Null();
}

void CaptureService::_devices_finalizer(Napi::Env env, CaptureService* self)
{

}
