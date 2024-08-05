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

#include "./args.h"

class SimpleRender
{
public:
#ifdef WIN32
    SimpleRender(Args& args,
                 HWND hwnd,
                 HINSTANCE hinstance,
                 std::function<void()> closed_callback);
#endif

    ~SimpleRender();

    void SetTitle(std::string title);
    bool OnVideoFrame(VideoFrame* frame);
    bool OnAudioFrame(AudioFrame* frame);
    void OnClose();
    void Clear();

    bool IsRender = true;
private:
    Args& _args;
    bool _runing = true;
    std::function<void()> _callback;
    WindowHandle _window_handle = nullptr;
    Render _renderer = nullptr;
#ifdef WIN32
    HWND _hwnd;
#endif
};

#endif