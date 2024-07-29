#ifndef RECEIVER_H
#define RECEIVER_H
#pragma once

#include <napi.h>
#include <thread>
#include <windows.h>

extern "C"
{
#include <mirror.h>
#include <renderer.h>
}

class ReceiverService : public Napi::ObjectWrap<ReceiverService>
{
public:
    static void Create(Napi::Env env, Napi::Object exports);

    ReceiverService(const Napi::CallbackInfo& info);

    Napi::Value Close(const Napi::CallbackInfo& info);
private:
    using Ref = Napi::Reference<Napi::Value>;

    static void _close_proc(void* ctx);
    static bool _video_proc(void* ctx, VideoFrame* frame);
    static bool _audio_proc(void* ctx, AudioFrame* frame);
    static LRESULT CALLBACK _wnd_proc(HWND hwnd,
                                      UINT message,
                                      WPARAM wparam,
                                      LPARAM lparam);
    static void _callback_proc(Napi::Env env,
                               Napi::Function callback,
                               Ref* context,
                               void* data);

    using ThreadSafeFunction = Napi::TypedThreadSafeFunction<Ref, void, _callback_proc>;

    ThreadSafeFunction _callback;
    std::thread* _thread = nullptr;
    Receiver _receiver = nullptr;
    Render _renderer = nullptr;
};

#endif // RECEIVER_H