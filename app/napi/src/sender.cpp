#include "./sender.h"
#include "./context.h"

void SenderService::Create(Napi::Env env, Napi::Object exports)
{
    auto props =
    {
        InstanceMethod<&SenderService::SetMulticast>("set_multicast"),
        InstanceMethod<&SenderService::GetMulticast>("get_multicast"),
        InstanceMethod<&SenderService::Close>("close"),
    };

    exports.Set("SenderService", DefineClass(env, "SenderService", props));
}

SenderService::SenderService(const Napi::CallbackInfo& info) : Napi::ObjectWrap<SenderService>(info)
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
    sink.video = nullptr;
    sink.audio = nullptr;
    sink.close = SenderService::_close_proc;
    _sender = mirror_create_sender(context->mirror, id, sink);
    if (_sender == nullptr)
    {
        Napi::Error::New(env, "create sender failed").ThrowAsJavaScriptException();
        return;
    }
    else
    {
        _callback = Napi::Persistent(info[1].As<Napi::Function>());
    }
}

Napi::Value SenderService::Close(const Napi::CallbackInfo& info)
{
    auto env = info.Env();
    if (_sender != nullptr)
    {
        mirror_sender_destroy(_sender);
        _sender = nullptr;
    }

    return env.Null();
}

Napi::Value SenderService::SetMulticast(const Napi::CallbackInfo& info)
{
    auto env = info.Env();
    if (_sender == nullptr)
    {
        Napi::Error::New(env, "sender is null").ThrowAsJavaScriptException();
        return env.Null();
    }

    if (info.Length() != 1 || !info[0].IsBoolean())
    {
        Napi::TypeError::New(env, "invalid arguments").ThrowAsJavaScriptException();
        return env.Null();
    }

    mirror_sender_set_multicast(_sender, info[0].As<Napi::Boolean>());
    return env.Null();
}

Napi::Value SenderService::GetMulticast(const Napi::CallbackInfo& info)
{
    auto env = info.Env();
    return Napi::Boolean::From(env, mirror_sender_get_multicast(_sender));
}

void SenderService::_close_proc(void* ctx)
{
    auto self = (SenderService*)ctx;
    self->_callback.Call({});
}