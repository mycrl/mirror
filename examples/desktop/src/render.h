#ifndef RENDER_H
#define RENDER_H
#pragma once

#ifdef WIN32
#include <windows.h>
#endif

extern "C"
{
#include <renderer.h>
}

#include <functional>
#include <SDL_events.h>

#include "./args.h"

class SimpleRender
{
public:
#ifdef WIN32
    SimpleRender(Args& args,
                 HWND hwnd,
                 HINSTANCE hinstance,
                 std::function<void()> closed_callback);
#else
    SimpleRender(Args& args, std::function<void()> closed_callback);
#endif

    ~SimpleRender();

    void SetTitle(std::string title);
    bool OnVideoFrame(VideoFrame* frame);
    bool OnAudioFrame(AudioFrame* frame);
    void OnClose();
    void Clear();
    void RunEventLoop(std::function<bool(SDL_Event*)> handler);

    bool IsRender = true;
private:
    Args& _args;
    bool _runing = true;
    std::function<void()> _callback;
    Render _renderer = nullptr;
#ifdef WIN32
    WindowHandle _window_handle = nullptr;
    HWND _hwnd;
#endif
};

#endif
