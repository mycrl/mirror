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
    _window_handle = create_window_handle_for_win32(hwnd, args.ArgsParams.width, args.ArgsParams.height);
    _renderer = renderer_create(_window_handle, RENDER_BACKEND_DX11);
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

    SetWindowText(_hwnd, base.c_str());
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
