//
//  camera.h
//  capture
//
//  Created by Panda on 2024/6/30.
//

#pragma once
#ifdef WIN32

#include <windows.h>
#include <mfapi.h>
#include <mfidl.h>
#include <mfreadwrite.h>
#include <mftransform.h>
#include <shlwapi.h>

#include <thread>
#include <functional>

#include "capture.h"

class CameraCapture
{
public:
    CameraCapture();
    ~CameraCapture();

    static int EnumDevices(struct DeviceList* list);
    int StartCapture(const char* id,
                     int width,
                     int height,
                     int fps,
                     std::function<void(VideoFrame*)> callback);
    void StopCapture();
private:
    bool _is_runing = false;

    bool _ReadSample(IMFSourceReader* reader,
                     VideoFrame* frame,
                     std::function<void(VideoFrame*)> callback);
    static char* _WcharToString(WCHAR* src, int size);
};

#endif // WIN32