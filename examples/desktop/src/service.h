#ifndef SERVICE_H
#define SERVICE_H
#pragma once

extern "C"
{
#include <mirror.h>
}

#include "./args.h"
#include "./render.h"

class MirrorServiceExt
{
public:
#ifdef WIN32
    MirrorServiceExt(Args& args, HWND hwnd);
#else
    MirrorServiceExt(Args& args, uint64_t window_handle);
#endif

    ~MirrorServiceExt();

    bool CreateMirrorSender();
    bool CreateMirrorReceiver();
    void Close();

    SimpleRender* Render = nullptr;
private:
    Args& _args;
    Mirror _mirror = nullptr;
    Sender _sender = nullptr;
    Receiver _receiver = nullptr;
    bool _is_runing = true;
};

#endif
