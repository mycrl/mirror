#ifndef RENDER_H
#define RENDER_H
#pragma once

#ifdef WIN32
#include <windows.h>
#endif

extern "C"
{
#include <mirror.h>
}

#include <functional>

#ifdef LINUX
#include <SDL_events.h>
#endif // LINUX

#include "./args.h"

class SimpleRender
{
public:
#ifdef WIN32
    SimpleRender(Args& args, HWND hwnd);
#else
    SimpleRender(Args& args, uint64_t window_handle);
#endif

    ~SimpleRender();

    void SetTitle(std::string title);
    bool OnVideoFrame(VideoFrame* frame);
    bool OnAudioFrame(AudioFrame* frame);
    void Close();
    void Create();

    bool IsRender = true;
private:
    Args& _args;
    Render _renderer = nullptr;
    WindowHandle _window_handle = nullptr;

#ifdef WIN32
    HWND _hwnd;
#endif
};

#endif
