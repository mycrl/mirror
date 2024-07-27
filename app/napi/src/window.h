#ifndef WINDOW_H
#define WINDOW_H
#pragma once

#include <windows.h>
#include <functional>

extern "C"
{
#include <renderer.h>
}

class IWindow
{
public:
    IWindow()
    {
    }

    void Create(int width,
                int height,
                std::function<void(WindowHandle)> callback)
    {
        HINSTANCE hinstance = (HINSTANCE)GetModuleHandle(nullptr);
        WNDCLASSEX wcex;
        wcex.cbSize = sizeof(WNDCLASSEX);
        wcex.style = CS_HREDRAW | CS_VREDRAW;
        wcex.lpfnWndProc = _wnd_proc;
        wcex.cbClsExtra = 0;
        wcex.cbWndExtra = 0;
        wcex.hInstance = hinstance;
        wcex.hIcon = LoadIcon(hinstance, IDI_APPLICATION);
        wcex.hCursor = LoadCursor(nullptr, IDC_ARROW);
        wcex.hbrBackground = (HBRUSH)(COLOR_WINDOW + 1);
        wcex.lpszMenuName = nullptr;
        wcex.lpszClassName = "mirror remote casting frame";
        wcex.hIconSm = LoadIcon(wcex.hInstance, IDI_APPLICATION);
        if (!RegisterClassEx(&wcex))
        {
            callback(nullptr);
            return;
        }

        _hwnd = CreateWindow("mirror remote casting frame",
                             "mirror remote casting frame",
                             WS_OVERLAPPEDWINDOW | WS_MAXIMIZE,
                             0,
                             0,
                             width,
                             height,
                             nullptr,
                             nullptr,
                             hinstance,
                             nullptr);
        if (!_hwnd)
        {
            callback(nullptr);
            return;
        }

        auto window_handle = renderer_create_window_handle(_hwnd, hinstance);
        if (window_handle)
        {
            callback(window_handle);
        }

        ShowWindow(_hwnd, SW_SHOW);
        UpdateWindow(_hwnd);

        MSG msg;
        while (GetMessage(&msg, NULL, 0, 0))
        {
            TranslateMessage(&msg);
            DispatchMessage(&msg);
        }

        DestroyWindow(_hwnd);
        _hwnd = nullptr;
    }
private:
    HWND _hwnd = nullptr;

    static LRESULT CALLBACK _wnd_proc(HWND hwnd, UINT message, WPARAM wparam, LPARAM lparam)
    {
        switch (message)
        {
            case WM_DESTROY:
                PostQuitMessage(0);
                break;
            default:
                return DefWindowProc(hwnd, message, wparam, lparam);
                break;
        }

        return 0;
    }
};

#endif // WINDOW_H