#include "./render.h"

#ifdef WIN32
SimpleRender::SimpleRender(Args& args,
                           HWND hwnd,
                           std::function<void()> closed_callback)
    : _callback(closed_callback)
    , _args(args)
    , _hwnd(hwnd)
{
    
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

bool SimpleRender::OnVideoFrame(struct VideoFrame* frame)
{
    return true;
}

bool SimpleRender::OnAudioFrame(struct AudioFrame* frame)
{
    return true;
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