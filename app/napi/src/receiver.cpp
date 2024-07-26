#include "./receiver.h"
#include "./context.h"

void ReceiverService::Create(Napi::Env env, Napi::Object exports)
{
    auto props =
    {
        InstanceMethod<&ReceiverService::Close>("close"),
    };

    exports.Set("ReceiverService", DefineClass(env, "ReceiverService", props));
}

ReceiverService::ReceiverService(const Napi::CallbackInfo& info) : Napi::ObjectWrap<ReceiverService>(info)
{
    auto env = info.Env();
    auto context = env.GetInstanceData<Context>();
    auto id = info[0].As<Napi::Number>().Uint32Value();

    if (context->mirror == nullptr)
    {
        Napi::Error::New(env, "mirror is null").ThrowAsJavaScriptException();
        return;
    }

    FrameSink sink;
    sink.ctx = this;
    sink.video = ReceiverService::_video_proc;
    sink.audio = ReceiverService::_audio_proc;
    sink.close = ReceiverService::_close_proc;
    _receiver = mirror_create_receiver(context->mirror, id, sink);
    if (_receiver == nullptr)
    {
        Napi::Error::New(env, "create receiver failed").ThrowAsJavaScriptException();
        return;
    }
    else
    {
        _callback = Napi::Persistent(info[1].As<Napi::Function>());
    }
}

Napi::Value ReceiverService::Close(const Napi::CallbackInfo& info)
{
    auto env = info.Env();
    if (_receiver != nullptr)
    {
        mirror_receiver_destroy(_receiver);
        _receiver = nullptr;
    }

    return env.Null();
}

bool ReceiverService::_video_proc(void* ctx, VideoFrame* frame)
{
    return true;
}

bool ReceiverService::_audio_proc(void* ctx, AudioFrame* frame)
{
    return true;
}

void ReceiverService::_close_proc(void* ctx)
{
    auto self = (ReceiverService*)ctx;
    //self->_callback.Call({});
}