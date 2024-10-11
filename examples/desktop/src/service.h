#ifndef SERVICE_H
#define SERVICE_H
#pragma once

extern "C"
{
#include <mirror.h>
}

#include "./args.h"

class MirrorServiceExt
{
public:
    MirrorServiceExt(Args& args);
    ~MirrorServiceExt();

    bool CreateMirrorSender(Render render);
    bool CreateMirrorReceiver(Render render);
    void Close();
private:
    Args& _args;
    Mirror _mirror = nullptr;
    Sender _sender = nullptr;
    Receiver _receiver = nullptr;
    bool _is_runing = true;
};

#endif
