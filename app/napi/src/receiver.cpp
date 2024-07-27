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
    auto size = info[1].As<Napi::Object>();

    _size.width = size.Get("width").Unwrap().As<Napi::Number>().Uint32Value();
    _size.height = size.Get("height").Unwrap().As<Napi::Number>().Uint32Value();

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
        _callback = Napi::Persistent(info[2].As<Napi::Function>());
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
    auto self = (ReceiverService*)ctx;
    if (self->_status == RenderStatus::New)
    {
        self->_status = RenderStatus::Createing;
        self->_thread = new std::thread(
            [=]()
            {
                self->_window.Create(self->_size.width, 
                                     self->_size.height, 
                                     [=](WindowHandle window_handle) 
                                     {
                                         if (window_handle)
                                         {
                                             self->_renderer = renderer_create(self->_size, window_handle);
                                             self->_status = RenderStatus::Created;
                                         }
                                     });

                self->_status = RenderStatus::New;
            });
    }
    else if (self->_status == RenderStatus::Created)
    {
        if (self->_renderer) {
            return renderer_on_video(self->_renderer, frame);
        }
    }

    return true;
}

bool ReceiverService::_audio_proc(void* ctx, AudioFrame* frame)
{
    auto self = (ReceiverService*)ctx;
    if (self->_renderer) {
        return renderer_on_audio(self->_renderer, frame);
    }

    return true;
}

void ReceiverService::_close_proc(void* ctx)
{
    auto self = (ReceiverService*)ctx;
    if (self->_renderer) {
        renderer_destroy(self->_renderer);
    }

    //self->_callback.Call({});
}