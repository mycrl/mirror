#ifndef SERVICE_H
#define SERVICE_H
#pragma once

#include "./args.h"
#include "./render.h"
#include "./wrapper.h"

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
    CaptureSettings _settings;
    SimpleRender* _render = nullptr;
    MirrorService* _mirror = nullptr;
    std::optional<MirrorSender> _sender = std::nullopt;
    std::optional<MirrorReceiver> _receiver = std::nullopt;
};

#endif