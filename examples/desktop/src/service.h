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
    MirrorServiceExt(Args& args, HWND hwnd, HINSTANCE hinstance);
#endif

    ~MirrorServiceExt();

    bool CreateMirrorSender();
    bool CreateMirrorReceiver();
    void Close();
private:
    Args& _args;
    Mirror _mirror = nullptr;
    Sender _sender = nullptr;
    Receiver _receiver = nullptr;
    SimpleRender* _render = nullptr;
    bool is_created = false;

    bool _create_mirror();
};

#endif