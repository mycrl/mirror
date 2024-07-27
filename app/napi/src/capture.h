#ifndef CAPTURE_H
#define CAPTURE_H
#pragma once

#include <optional>
#include <string>
#include <napi.h>

extern "C"
{
#include <mirror.h>
}

class CaptureService : public Napi::ObjectWrap<CaptureService>
{
public:
    static void Create(Napi::Env env, Napi::Object exports);

    CaptureService(const Napi::CallbackInfo& info);

    Napi::Value StartCapture(const Napi::CallbackInfo& info);
    Napi::Value StopCapture(const Napi::CallbackInfo& info);
    Napi::Value GetDevices(const Napi::CallbackInfo& info);
    Napi::Value SetInputDevice(const Napi::CallbackInfo& info);
private:
    std::optional<Devices> _devices = std::nullopt;

    static void _devices_finalizer(Napi::Env env, CaptureService* self);
};

#endif // CAPTURE_H