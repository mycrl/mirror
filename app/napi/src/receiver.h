#ifndef RECEIVER_H
#define RECEIVER_H
#pragma once

#include <napi.h>

extern "C"
{
#include <mirror.h>
}

class ReceiverService : public Napi::ObjectWrap<ReceiverService>
{
public:
    static void Create(Napi::Env env, Napi::Object exports);

    ReceiverService(const Napi::CallbackInfo& info);
    
    Napi::Value Close(const Napi::CallbackInfo& info);
private:
    Receiver _receiver = nullptr;
    Napi::FunctionReference _callback;

    static bool _video_proc(void* ctx, VideoFrame* frame);
    static bool _audio_proc(void* ctx, AudioFrame* frame);
    static void _close_proc(void* ctx);
};

#endif // RECEIVER_H