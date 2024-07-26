#ifndef SENDER_H
#define SENDER_H
#pragma once

#include <napi.h>

extern "C"
{
#include <mirror.h>
}

class SenderService : public Napi::ObjectWrap<SenderService>
{
public:
    static void Create(Napi::Env env, Napi::Object exports);

    SenderService(const Napi::CallbackInfo& info);

    Napi::Value Close(const Napi::CallbackInfo& info);
    Napi::Value SetMulticast(const Napi::CallbackInfo& info);
    Napi::Value GetMulticast(const Napi::CallbackInfo& info);
private:
    Sender _sender = nullptr;
    Napi::FunctionReference _callback;

    static void _close_proc(void* ctx);
};

#endif // SENDER_H
