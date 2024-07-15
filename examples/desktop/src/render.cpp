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
#endif

SimpleRender::~SimpleRender()
{
    _runing = false;
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

    SetWindowText(_hwnd, base.c_str());
}

bool SimpleRender::OnVideoFrame(VideoFrame* frame)
{
    if (!IsRender)
    {
        return;
    }
    
    return renderer_on_video(_renderer, frame);
}

bool SimpleRender::OnAudioFrame(AudioFrame* frame)
{
    if (!IsRender)
    {
        return;
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
