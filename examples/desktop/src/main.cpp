//
//  main.cpp
//  sender
//
//  Created by Panda on 2024/4/13.
//

#ifdef WIN32
#include <windows.h>
#endif

#ifndef WIN32
#include <SDL.h>
#include <SDL_syswm.h>
#endif

#include "./args.h"
#include "./service.h"

extern "C"
{
#include <mirror.h>
}

static MirrorServiceExt* mirror_service = nullptr;

#ifdef WIN32
LRESULT CALLBACK window_handle_proc(HWND hwnd,
                                    UINT message,
                                    WPARAM wparam,
                                    LPARAM lparam)
{
    switch (message)
    {
    case WM_CLOSE:
        PostQuitMessage(0);
        return 0;
    case WM_KEYDOWN:
        switch (wparam)
        {
        case 'S':
            if (!mirror_service->CreateMirrorSender())
            {
                MessageBox(nullptr,
                           TEXT("Failed to create sender"),
                           TEXT("Error"),
                           0);
            }

            break;
        case 'R':
            if (!mirror_service->CreateMirrorReceiver())
            {
                MessageBox(nullptr,
                           TEXT("Failed to create receiver"),
                           TEXT("Error"),
                           0);
            }

            break;
        case 'K':
            mirror_service->Close();
            break;
        default:
            break;
        }

        return 0;
    default:
        return DefWindowProc(hwnd, message, wparam, lparam);
    }
}

int WinMain(HINSTANCE hinstance,
            HINSTANCE _prev_instance,
            LPSTR cmd_line,
            int _show_cmd)
{
    AttachConsole(ATTACH_PARENT_PROCESS);
    freopen("CONIN$", "r+t", stdin);
    freopen("CONOUT$", "w+t", stdout);

    WNDCLASS wc;
    wc.style = CS_OWNDC;
    wc.lpfnWndProc = window_handle_proc;
    wc.cbClsExtra = 0;
    wc.cbWndExtra = 0;
    wc.hInstance = hinstance;
    wc.hIcon = LoadIcon(nullptr, IDI_APPLICATION);
    wc.hCursor = LoadCursor(nullptr, IDC_ARROW);
    wc.hbrBackground = (HBRUSH)GetStockObject(BLACK_BRUSH);
    wc.lpszMenuName = nullptr;
    wc.lpszClassName = "example";

    RegisterClass(&wc);

    Args args = Args(std::string(cmd_line));
    int height = (GetSystemMetrics(SM_CYFRAME) +
                  GetSystemMetrics(SM_CYCAPTION) +
                  GetSystemMetrics(SM_CXPADDEDBORDER));
    HWND hwnd = CreateWindow("example",
                             "example",
                             WS_CAPTION | WS_POPUPWINDOW | WS_VISIBLE,
                             0,
                             0,
                             args.ArgsParams.width,
                             args.ArgsParams.height + height,
                             nullptr,
                             nullptr,
                             hinstance,
                             nullptr);
    mirror_service = new MirrorServiceExt(args, hwnd);

    MSG message;
    while (GetMessage(&message, nullptr, 0, 0))
    {
        TranslateMessage(&message);
        DispatchMessage(&message);
    }

    DestroyWindow(hwnd);

    delete mirror_service;
    return 0;
}

#else

int main(int argc, char* argv[])
{
    mirror_startup();

    Args args = Args(argc >= 2 ? std::string(argv[1]) : "");

    SDL_Init(SDL_INIT_EVENTS);
#ifdef LINUX
    SDL_Window* window = SDL_CreateWindow("example",
                                          0,
                                          0,
                                          args.ArgsParams.width,
                                          args.ArgsParams.height,
                                          SDL_WINDOW_VULKAN);
#else
    SDL_Window* window = SDL_CreateWindow("example",
                                          0,
                                          0,
                                          args.ArgsParams.width,
                                          args.ArgsParams.height,
                                          SDL_WINDOW_METAL);   
#endif

    SDL_SysWMinfo info;
    SDL_VERSION(&info.version);
    SDL_GetWindowWMInfo(window, &info);

#ifdef LINUX
    auto window_handle = create_window_handle_for_xlib(info.info.x11.window, 
                                                       info.info.x11.display,
                                                       args.ArgsParams.width, 
                                                       args.ArgsParams.height);
#else
    auto window_handle = create_window_handle_for_appkit(info.info.cocoa.window, 
                                                        args.ArgsParams.width, 
                                                        args.ArgsParams.height);
#endif

    auto renderer = renderer_create(window_handle, RENDER_BACKEND_WGPU);
    mirror_service = new MirrorServiceExt(args);

    SDL_Event event;
    while (SDL_WaitEvent(&event) == 1) {
        if (event.type == SDL_QUIT)
        {
            break;
        }
        else if (event.type == SDL_KEYDOWN)
        {
            switch (event.key.keysym.sym)
            {
            case SDLK_r:
                mirror_service->CreateMirrorReceiver(window_handle);

                break;
            case SDLK_s:
                mirror_service->CreateMirrorSender(window_handle);

                break;
            case SDLK_k:
                mirror_service->Close();

                break;
            }
        }
    }

    mirror_shutdown();
    renderer_destroy(renderer);
    window_handle_destroy(window_handle);
    SDL_DestroyWindow(window);
    SDL_Quit();
}

#endif
