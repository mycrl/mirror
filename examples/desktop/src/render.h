#ifndef RENDER_H
#define RENDER_H
#pragma once

#ifdef WIN32
#include <windows.h>
#endif

#include <functional>

#include "./args.h"
#include "./wrapper.h"

class SimpleRender : public AVFrameSink
{
public:
#ifdef WIN32
    SimpleRender(Args& args,
                 HWND hwnd,
                 std::function<void()> closed_callback);
#endif

    ~SimpleRender();

    void SetTitle(std::string title);
    bool OnVideoFrame(struct VideoFrame* frame);
    bool OnAudioFrame(struct AudioFrame* frame);
    void OnClose();
    void Clear();

    bool IsRender = true;
private:
    Args& _args;
    bool _runing = true;
    std::function<void()> _callback;

#ifdef WIN32
    HWND _hwnd;
#endif
};

#endif