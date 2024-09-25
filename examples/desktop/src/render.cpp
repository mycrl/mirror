#include "./render.h"

#ifdef WIN32
SimpleRender::SimpleRender(Args& args, HWND hwnd)
    : _args(args)
    , _hwnd(hwnd)
{
}
#else
SimpleRender::SimpleRender(Args& args)
    : _args(args)
{

    Size size;
    size.width = args.ArgsParams.width;
    size.height = args.ArgsParams.height;

    _renderer = renderer_create(size, nullptr);
}
#endif

SimpleRender::~SimpleRender()
{
    Close();
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
    
    /*if (!IsRender)
    {
        return true;
    }*/

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

void SimpleRender::Close()
{
    if (_renderer == nullptr)
    {
        return;
    }

    renderer_destroy(_renderer);
    _renderer = nullptr;

    SetTitle("");
}

void SimpleRender::Create()
{
    if (_renderer != nullptr)
    {
        return;
    }

    _renderer = renderer_create(_hwnd);
}

#ifdef LINUX
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
#endif // LINUX
