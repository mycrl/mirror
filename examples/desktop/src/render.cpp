#include "./render.h"

#ifdef WIN32
SimpleRender::SimpleRender(Args& args,
                           HWND hwnd,
                           HINSTANCE hinstance,
                           std::function<void()> closed_callback)
    : _callback(closed_callback)
    , _args(args)
    , _hwnd(hwnd)
{
    Size size;
    size.width = args.ArgsParams.width;
    size.height = args.ArgsParams.height;

    _window_handle = renderer_create_window_handle(hwnd, hinstance);
    _renderer = renderer_create(size, _window_handle);
}
#else
SimpleRender::SimpleRender(Args& args, std::function<void()> closed_callback)
    : _callback(closed_callback)
    , _args(args)
{

    Size size;
    size.width = args.ArgsParams.width;
    size.height = args.ArgsParams.height;

    _renderer = renderer_create(size, nullptr);
}
#endif

SimpleRender::~SimpleRender()
{
    _runing = false;
    renderer_destroy(_renderer);
}

void SimpleRender::SetTitle(std::string title)
{
    std::string base = "example - s/create sender, r/create receiver, k/stop";
    if (title.length() > 0)
    {
        base += " - [";
        base += title;
        base += "]";
    }

#ifdef WIN32
    SetWindowText(_hwnd, base.c_str());
#endif
}

bool SimpleRender::OnVideoFrame(VideoFrame* frame)
{
    if (_renderer == nullptr)
    {
        return false;
    }
    
    if (!IsRender)
    {
        return true;
    }

    if (!IsRender)
    {
        return true;
    }

    return renderer_on_video(_renderer, frame);
}

bool SimpleRender::OnAudioFrame(AudioFrame* frame)
{
    if (_renderer == nullptr)
    {
        return false;
    }

    if (!IsRender)
    {
        return true;
    }

    return renderer_on_audio(_renderer, frame);
}

void SimpleRender::OnClose()
{
    _callback();
    SetTitle("");
    Clear();
}

void SimpleRender::Clear()
{

}

struct EventLoopContext
{
    std::function<bool(SDL_Event*)> func;
};

bool event_proc(const void* event, void* ctx)
{
    auto ctx_ = (EventLoopContext*)ctx;
    return ctx_->func((SDL_Event*)event);
}

void SimpleRender::RunEventLoop(std::function<bool(SDL_Event*)> handler)
{
    auto ctx = new EventLoopContext{};
    ctx->func = handler;
    renderer_event_loop(_renderer, event_proc, ctx);
}
