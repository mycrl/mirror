//
//  desktop.h
//  capture
//
//  Created by Panda on 2024/7/12.
//

#pragma once
#ifdef WIN32

#include <windows.h>
#include <thread>
#include <string>
#include <vector>
#include <functional>

#include "./capture.h"

class GDICapture
{
public:
    GDICapture();
    ~GDICapture();

    int EnumDevices(struct DeviceList* list);
    int StartCapture(const char* id,
                     int width,
                     int height,
                     int fps,
                     std::function<void(VideoFrame*)> callback);
    void StopCapture();
private:
    bool _is_runing = false;
    VideoFrame _frame = {};
    std::vector<std::string> _devices = {};

    static BOOL CALLBACK _monitor_enum_proc(HMONITOR monitor,
                                            HDC screen,
                                            LPRECT rect,
                                            LPARAM ctx);
    HBITMAP _get_screen_bmp(HDC screen);
    void _release_frame();
};

#endif