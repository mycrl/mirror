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
    SimpleRender(Args& args);
#endif

    ~SimpleRender();

    void SetTitle(std::string title);
    bool OnVideoFrame(VideoFrame* frame);
    bool OnAudioFrame(AudioFrame* frame);
    void Close();
    void Create();

#ifdef LINUX
    void RunEventLoop(std::function<bool(SDL_Event*)> handler);
#endif // LINUX

    bool IsRender = true;
private:
    Args& _args;
    Render _renderer = nullptr;
#ifdef WIN32
    WindowHandle _window_handle = nullptr;
    HWND _hwnd;
#endif
};

#endif
