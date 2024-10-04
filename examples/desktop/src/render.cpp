#include "./render.h"

#ifdef WIN32
SimpleRender::SimpleRender(Args& args, HWND hwnd)
    : _args(args)
    , _hwnd(hwnd)
{
    RECT rect;
    GetClientRect(hwnd, &rect);
    int width = rect.right - rect.left;
    int height = rect.bottom - rect.top;
    _window_handle = create_window_handle_for_win32(hwnd, width, height);
}
#else
SimpleRender::SimpleRender(Args& args, uint64_t window_handle)
    : _args(args)
{
    _window_handle = create_window_handle_for_xlib(window_handle, args.ArgsParams.width, args.ArgsParams.height);
}
#endif

SimpleRender::~SimpleRender()
{
    Close();

    if (_window_handle != nullptr)
    {
        window_handle_destroy(_window_handle);
        _window_handle = nullptr;
    }
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

    _renderer = renderer_create(_window_handle, xVideoRenderBackendWgpu);
}
