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
    using Ref = Napi::Reference<Napi::Value>;

    static void _close_proc(void* ctx);
    static void _callback_proc(Napi::Env env,
                               Napi::Function callback,
                               Ref* context,
                               void* data);

    using ThreadSafeFunction = Napi::TypedThreadSafeFunction<Ref, void, _callback_proc>;

    Sender _sender = nullptr;
    ThreadSafeFunction _callback;
};

#endif // SENDER_H
