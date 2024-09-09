#include "./lib.h"
#include "./capture.h"
#include "./sender.h"
#include "./context.h"
#include "./receiver.h"

void MirrorService::Create(Napi::Env env, Napi::Object exports)
{
    auto props =
    {
        InstanceMethod<&MirrorService::Init>("init"),
        InstanceMethod<&MirrorService::Quit>("quit"),
        InstanceMethod<&MirrorService::CreateSender>("create_sender"),
        InstanceMethod<&MirrorService::CreateReceiver>("create_receiver"),
        InstanceMethod<&MirrorService::CreateCaptureService>("create_capture_service"),
    };

    exports.Set("MirrorService", DefineClass(env, "MirrorService", props));
}

MirrorService::MirrorService(const Napi::CallbackInfo& info) : Napi::ObjectWrap<MirrorService>(info)
{
}

Napi::Value MirrorService::Init(const Napi::CallbackInfo& info)
{
    auto env = info.Env();
    if (info.Length() != 1 || !info[0].IsObject())
    {
        Napi::TypeError::New(env, "invalid arguments").ThrowAsJavaScriptException();
        return Napi::Boolean::From(env, false);
    }

    auto object = info[0].As<Napi::Object>();
    _multicast = std::string(object.Get("multicast").Unwrap().As<Napi::String>());
    _encoder = std::string(object.Get("encoder").Unwrap().As<Napi::String>());
    _decoder = std::string(object.Get("decoder").Unwrap().As<Napi::String>());
    _server = std::string(object.Get("server").Unwrap().As<Napi::String>());

    MirrorDescriptor options;
    options.video.encoder = const_cast<char*>(_encoder.c_str());
    options.video.decoder = const_cast<char*>(_decoder.c_str());
    options.video.width = object.Get("width").Unwrap().As<Napi::Number>().Uint32Value();
    options.video.height = object.Get("height").Unwrap().As<Napi::Number>().Uint32Value();
    options.video.frame_rate = object.Get("fps").Unwrap().As<Napi::Number>().Uint32Value();
    options.video.bit_rate = object.Get("bit_rate").Unwrap().As<Napi::Number>().Uint32Value();
    options.mtu = object.Get("mtu").Unwrap().As<Napi::Number>().Uint32Value();
    options.multicast = const_cast<char*>(_multicast.c_str());
    options.server = const_cast<char*>(_server.c_str());
    options.video.key_frame_interval = 21;
    options.audio.sample_rate = 48000;
    options.audio.bit_rate = 64000;

    if (!mirror_init(options))
    {
        Napi::Error::New(env, "initialization failed").ThrowAsJavaScriptException();
        return Napi::Boolean::From(env, false);
    }

    auto mirror = mirror_create();
    if (mirror == nullptr)
    {
        Napi::Error::New(env, "mirror create failed").ThrowAsJavaScriptException();
        return Napi::Boolean::From(env, false);
    }

    env.GetInstanceData<Context>()->mirror = mirror;
    return Napi::Boolean::From(env, true);
}

Napi::Value MirrorService::Quit(const Napi::CallbackInfo& info)
{
    auto env = info.Env();
    auto mirror = env.GetInstanceData<Context>()->mirror;
    if (mirror != nullptr)
    {
        mirror_destroy(mirror);
    }

    mirror_quit();
    return info.Env().Undefined();
}

Napi::Value MirrorService::CreateCaptureService(const Napi::CallbackInfo& info)
{
    auto env = info.Env();
    auto context = env.GetInstanceData<Context>();
    auto func = context->exports.Get("CaptureService").Unwrap();
    return func.As<Napi::Function>().New({}).Unwrap();
}

Napi::Value MirrorService::CreateSender(const Napi::CallbackInfo& info)
{
    auto env = info.Env();
    if (info.Length() != 2 || !info[0].IsNumber() || !info[1].IsFunction())
    {
        Napi::TypeError::New(env, "invalid arguments").ThrowAsJavaScriptException();
        return env.Null();
    }

    auto context = env.GetInstanceData<Context>();
    auto func = context->exports.Get("SenderService").Unwrap();
    return func.As<Napi::Function>().New({ info[0], info[1] }).Unwrap();
}

Napi::Value MirrorService::CreateReceiver(const Napi::CallbackInfo& info)
{
    auto env = info.Env();
    if (info.Length() != 2 || !info[0].IsNumber() || !info[1].IsFunction())
    {
        Napi::TypeError::New(env, "invalid arguments").ThrowAsJavaScriptException();
        return env.Null();
    }

    auto context = env.GetInstanceData<Context>();
    auto func = context->exports.Get("ReceiverService").Unwrap();
    return func.As<Napi::Function>().New({ info[0], info[1] }).Unwrap();
}

Napi::Object Init(Napi::Env env, Napi::Object exports)
{
    CaptureService::Create(env, exports);
    MirrorService::Create(env, exports);
    SenderService::Create(env, exports);
    ReceiverService::Create(env, exports);

    auto context = new Context();
    context->exports = Napi::Persistent(exports);
    env.SetInstanceData(context, Context::Finalize);
    return exports;
}

NODE_API_MODULE(mirror, Init)
