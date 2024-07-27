//
//  desktop.cpp
//  capture
//
//  Created by Panda on 2024/6/30.
//


#ifdef WIN32

#include "./desktop.h"

#include <libyuv.h>

GDICapture::GDICapture()
{
}

GDICapture::~GDICapture()
{
    _is_runing = false;
    this->_release_frame();
}

int GDICapture::EnumDevices(DeviceList* list)
{
    _devices.clear();
    if (!EnumDisplayMonitors(nullptr,
                             nullptr,
                             GDICapture::_monitor_enum_proc,
                             (LPARAM)this))
    {
        return -1;
    }

    for (auto& item : _devices)
    {
        DeviceDescription* device = new DeviceDescription{};
        device->type = DeviceType::kDeviceTypeScreen;
        device->name = item.c_str();
        device->id = item.c_str();

        list->devices[list->size] = device;
        list->size++;
    }

    return 0;
}

int GDICapture::StartCapture(const char* id,
                             int width,
                             int height,
                             int fps,
                             std::function<void(VideoFrame*)> callback)
{
    this->_release_frame();
    _frame.rect.width = width;
    _frame.rect.height = height;
    _frame.linesize[0] = width;
    _frame.linesize[1] = width;
    _frame.data[0] = new uint8_t[width * height * 1.5];
    _frame.data[1] = _frame.data[0] + (width * height);

    _is_runing = true;
    std::thread(
        [=]
        {
            HDC screen = CreateDC(TEXT(id), nullptr, nullptr, nullptr);
            int screen_width = GetDeviceCaps(screen, HORZRES);
            int screen_height = GetDeviceCaps(screen, VERTRES);

            uint8_t* argb = new uint8_t[width * height * 4];
            uint8_t* screen_rgb = new uint8_t[screen_width * screen_height * 3];
            uint8_t* screen_argb = new uint8_t[screen_width * screen_height * 4];

            for (; this->_is_runing;)
            {
                if (_frame.data[0] == nullptr)
                {
                    break;
                }

                HBITMAP hbitmap = _get_screen_bmp(screen);
                if (hbitmap == nullptr)
                {
                    break;
                }

                BITMAPINFO bi;
                bi.bmiHeader.biSize = sizeof(bi.bmiHeader);
                bi.bmiHeader.biWidth = screen_width;
                bi.bmiHeader.biHeight = -screen_height;
                bi.bmiHeader.biPlanes = 1;
                bi.bmiHeader.biBitCount = 24;
                bi.bmiHeader.biCompression = BI_RGB;
                bi.bmiHeader.biSizeImage = 0;
                bi.bmiHeader.biXPelsPerMeter = 0;
                bi.bmiHeader.biYPelsPerMeter = 0;
                bi.bmiHeader.biClrUsed = 0;
                bi.bmiHeader.biClrImportant = 0;

                if (GetDIBits(screen,
                              hbitmap,
                              0,
                              (UINT)screen_height,
                              screen_rgb,
                              &bi,
                              DIB_RGB_COLORS) == 0)
                {
                    break;
                }

                if (libyuv::RGB24ToARGB(screen_rgb,
                                        screen_width * 3,
                                        screen_argb,
                                        screen_width * 4,
                                        screen_width,
                                        screen_height) != 0)
                {
                    break;
                }

                if (libyuv::ARGBScale(screen_argb,
                                      screen_width * 4,
                                      screen_width,
                                      screen_height,
                                      argb,
                                      width * 4,
                                      width,
                                      height,
                                      libyuv::FilterMode::kFilterBilinear) != 0)
                {
                    break;
                }

                if (libyuv::ARGBToNV12(argb,
                                       width * 4,
                                       this->_frame.data[0],
                                       this->_frame.linesize[0],
                                       this->_frame.data[1],
                                       this->_frame.linesize[1],
                                       width,
                                       height) != 0)
                {
                    break;
                }

                callback(&this->_frame);

                DeleteObject(hbitmap);
                Sleep(1000 / fps);
            }

            ReleaseDC(nullptr, screen);
            delete[] screen_argb;
            delete[] screen_rgb;
            delete[] argb;
        }).detach();
    return 0;
}

void GDICapture::StopCapture()
{
    _is_runing = false;
}

BOOL CALLBACK GDICapture::_monitor_enum_proc(HMONITOR monitor,
                                             HDC screen,
                                             LPRECT rect,
                                             LPARAM ctx)
{
    auto self = (GDICapture*)ctx;

    MONITORINFOEX mi;
    mi.cbSize = sizeof(mi);
    if (!GetMonitorInfo(monitor, &mi))
    {
        return true;
    }

    self->_devices.push_back(std::string(mi.szDevice));
    return true;
}

void GDICapture::_release_frame()
{
    if (_frame.data[0] != nullptr)
    {
        delete[] _frame.data[0];
    }

    _frame.data[0] = nullptr;
    _frame.data[1] = nullptr;
}

HBITMAP GDICapture::_get_screen_bmp(HDC screen)
{
    HDC host = CreateCompatibleDC(screen);
    int width = GetDeviceCaps(screen, HORZRES);
    int height = GetDeviceCaps(screen, VERTRES);
    HBITMAP hbitmap = CreateCompatibleBitmap(screen, width, height);
    HGDIOBJ hold = SelectObject(host, hbitmap);
    if (!BitBlt(host, 0, 0, width, height, screen, 0, 0, SRCCOPY | CAPTUREBLT))
    {
        return nullptr;
    }

    SelectObject(host, hold);
    DeleteDC(host);
    return hbitmap;
}

#endif // WIN32
