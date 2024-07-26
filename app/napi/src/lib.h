#ifndef NATIVE_H
#define NATIVE_H
#pragma once

#include <napi.h>
#include <string>

extern "C"
{
#include <mirror.h>
}

class MirrorService : public Napi::ObjectWrap<MirrorService>
{
public:
    MirrorService(const Napi::CallbackInfo& info);

    static void Create(Napi::Env env, Napi::Object exports);

    Napi::Value Init(const Napi::CallbackInfo& info);
    Napi::Value Quit(const Napi::CallbackInfo& info);
    Napi::Value CreateSender(const Napi::CallbackInfo& info);
    Napi::Value CreateReceiver(const Napi::CallbackInfo& info);
    Napi::Value CreateCaptureService(const Napi::CallbackInfo& info);
private:
    std::string _server;
    std::string _encoder;
    std::string _decoder;
    std::string _multicast;
};

#endif // NATIVE_H