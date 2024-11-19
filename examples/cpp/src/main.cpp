//
//  main.cpp
//  sender
//
//  Created by Panda on 2024/4/13.
//

#include <string>
#include <vector>

#ifdef WIN32
#include <windows.h>
#endif

#ifndef WIN32
#include <SDL.h>
#include <SDL_syswm.h>
#endif

#ifdef __OBJC__
#import <Cocoa/Cocoa.h>
#endif

extern "C"
{
#include <hylarana.h>
}

#include "./common.h"

static HylaranaRender RENDER = nullptr;

class HylaranaService
{
public:
    HylaranaService()
    {
    }

    ~HylaranaService()
    {
        Close();
    }

    bool CreateHylaranaSender()
    {
        if (_sender != nullptr)
        {
            return true;
        }

        auto video_sources = hylarana_get_sources(SOURCE_TYPE_SCREEN);
        auto audio_sources = hylarana_get_sources(SOURCE_TYPE_AUDIO);

        HylaranaVideoDescriptor video_options;
        video_options.encoder.codec = OPTIONS.encoder;
        video_options.encoder.width = OPTIONS.width;
        video_options.encoder.height = OPTIONS.height;
        video_options.encoder.frame_rate = OPTIONS.fps;
        video_options.encoder.key_frame_interval = 21;
        video_options.encoder.bit_rate = 500 * 1024 * 8;

        for (int i = 0; i < video_sources.size; i++)
        {
            if (video_sources.items[i].is_default)
            {
                video_options.source = &video_sources.items[i];
            }
        }

        HylaranaAudioDescriptor audio_options;
        audio_options.encoder.sample_rate = 48000;
        audio_options.encoder.bit_rate = 64000;

        for (int i = 0; i < audio_sources.size; i++)
        {
            if (audio_sources.items[i].is_default)
            {
                audio_options.source = &audio_sources.items[i];
            }
        }

        HylaranaDescriptor transport = {};
        transport.address = const_cast<char*>(OPTIONS.address.c_str());
        transport.strategy = OPTIONS.strategy;
        transport.mtu = 1500;

        HylaranaSenderDescriptor options;
        options.video = &video_options;
        options.audio = &audio_options;
        options.transport = transport;

        HylaranaFrameSink sink;
        sink.close = HylaranaService::_close_proc;
        sink.audio = nullptr;
        sink.video = nullptr;
        sink.ctx = (void*)this;

        char id[255] = { 0 };
        _sender = hylarana_create_sender(id, options, sink);
        if (_sender == nullptr)
        {
            return false;
        }

        char strategy_str[2];
        sprintf(strategy_str, "%d", OPTIONS.strategy);

        HylaranaProperties properties = hylarana_create_properties();
        hylarana_properties_insert(properties, "address", OPTIONS.address.c_str());
        hylarana_properties_insert(properties, "strategy", strategy_str);
        hylarana_properties_insert(properties, "id", id);

        _discovery = hylarana_discovery_register(3456, properties);
        if (_discovery == nullptr)
        {
            return false;
        }

        hylarana_properties_destroy(properties);
        _is_runing = true;
        return true;
    }

    bool CreateHylaranaReceiver()
    {
        if (_receiver != nullptr)
        {
            return true;
        }

        _discovery = hylarana_discovery_query(_query_resolve, this);
        if (_discovery == nullptr)
        {
            return false;
        }

        _is_runing = true;
        return true;
    }

    void Close()
    {
        if (_is_runing)
        {
            _is_runing = false;
        }
        else
        {
            return;
        }

        if (_discovery != nullptr)
        {
            hylarana_discovery_destroy(_discovery);
            _discovery = nullptr;
        }

        if (_sender != nullptr)
        {
            hylarana_sender_destroy(_sender);
            _sender = nullptr;
        }

        if (_receiver != nullptr)
        {
            hylarana_receiver_destroy(_receiver);
            _receiver = nullptr;
        }
    }
private:
    HylaranaDiscovery _discovery = nullptr;
    HylaranaReceiver _receiver = nullptr;
    HylaranaSender _sender = nullptr;
    bool _is_runing = true;

    static bool _video_proc(void* _, HylaranaVideoFrame* frame)
    {
        return hylarana_renderer_on_video(RENDER, frame);
    }

    static bool _audio_proc(void* _, HylaranaAudioFrame* frame)
    {
        return hylarana_renderer_on_audio(RENDER, frame);
    }

    static void _close_proc(void* ctx)
    {
        auto hylarana = (HylaranaService*)ctx;
        hylarana->Close();
    }

    static void _query_resolve(void* ctx,
                               const char** addrs,
                               size_t addrs_size,
                               HylaranaProperties properties)
    {
        HylaranaService* self = (HylaranaService*)ctx;
        if (addrs_size == 0)
        {
            return;
        }

        char id[255] = { 0 };
        char addr[40] = { 0 };
        char strategy[5] = { 0 };
        hylarana_properties_get(properties, "id", id);
        hylarana_properties_get(properties, "address", addr);
        hylarana_properties_get(properties, "strategy", strategy);

        HylaranaFrameSink sink;
        sink.close = HylaranaService::_close_proc;
        sink.video = HylaranaService::_video_proc;
        sink.audio = HylaranaService::_audio_proc;
        sink.ctx = ctx;

        HylaranaDescriptor transport = {};
        transport.strategy = (HylaranaStrategy)std::stoi(std::string(strategy));
        transport.mtu = 1500;

        SocketAddr socket_addr = SocketAddr(std::string(addr));
        if (transport.strategy == STRATEGY_DIRECT)
        {
            socket_addr.SetIP(std::string(addrs[0]));
        }

        std::string address = socket_addr.ToString();
        transport.address = address.c_str();

        HylaranaReceiverDescriptor options;
        options.transport = transport;
        options.video = OPTIONS.decoder;

        self->_receiver = hylarana_create_receiver(id, options, sink);
    }
};

static HylaranaService* MIRROR_SERVICE = nullptr;

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
            if (!MIRROR_SERVICE->CreateHylaranaSender())
            {
                MessageBox(nullptr,
                           TEXT("Failed to create sender"),
                           TEXT("Error"),
                           0);
            }

            break;
        case 'R':
            if (!MIRROR_SERVICE->CreateHylaranaReceiver())
            {
                MessageBox(nullptr,
                           TEXT("Failed to create receiver"),
                           TEXT("Error"),
                           0);
            }

            break;
        case 'K':
            MIRROR_SERVICE->Close();
            break;
        default:
            break;
        }

        return 0;
    default:
        return DefWindowProc(hwnd, message, wparam, lparam);
    }
}

int WINAPI WinMain(HINSTANCE hinstance,
                   HINSTANCE _prev_instance,
                   LPSTR cmd_line,
                   int _show_cmd)
{
    AttachConsole(ATTACH_PARENT_PROCESS);
    freopen("CONIN$", "r+t", stdin);
    freopen("CONOUT$", "w+t", stdout);

    if (parse_argv(std::string(cmd_line)) != 0)
    {
        return -1;
    }

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

    int height = (GetSystemMetrics(SM_CYFRAME) +
                  GetSystemMetrics(SM_CYCAPTION) +
                  GetSystemMetrics(SM_CXPADDEDBORDER));
    HWND hwnd = CreateWindow("example",
                             "example",
                             WS_CAPTION | WS_POPUPWINDOW | WS_VISIBLE,
                             0,
                             0,
                             OPTIONS.width,
                             OPTIONS.height + height,
                             nullptr,
                             nullptr,
                             hinstance,
                             nullptr);

    auto window_handle = hylarana_create_window_handle_for_win32(hwnd,
                                                                 OPTIONS.width,
                                                                 OPTIONS.height);
    RENDER = hylarana_renderer_create(window_handle, RENDER_BACKEND_DX11);
    MIRROR_SERVICE = new HylaranaService();

    MSG message;
    while (GetMessage(&message, nullptr, 0, 0))
    {
        TranslateMessage(&message);
        DispatchMessage(&message);
    }

    MIRROR_SERVICE->Close();
    hylarana_renderer_destroy(RENDER);
    hylarana_window_handle_destroy(window_handle);
    DestroyWindow(hwnd);

    delete MIRROR_SERVICE;
    return 0;
}

#else

int main(int argc, char* argv[])
{
    if (parse_argv(argc >= 2 ? std::string(argv[1]) : ""))
    {
        return -1;
    }

    hylarana_startup();

    SDL_Init(SDL_INIT_EVENTS);
#ifdef LINUX
    SDL_Window* window = SDL_CreateWindow("example",
                                          0,
                                          0,
                                          OPTIONS.width,
                                          OPTIONS.height,
                                          SDL_WINDOW_VULKAN);
#else
    SDL_Window* window = SDL_CreateWindow("example",
                                          0,
                                          0,
                                          OPTIONS.width,
                                          OPTIONS.height,
                                          SDL_WINDOW_METAL);
#endif

    SDL_SysWMinfo info;
    SDL_VERSION(&info.version);
    SDL_GetWindowWMInfo(window, &info);

#ifdef __OBJC__
    NSWindow* ns_window = (NSWindow*)info.info.cocoa.window;
    NSView* ns_view = [ns_window contentView];
    auto window_handle = hylarana_create_window_handle_for_appkit(ns_view,
                                                                  OPTIONS.width,
                                                                  OPTIONS.height);
#endif

#ifdef LINUX
    auto window_handle = hylarana_create_window_handle_for_xlib(info.info.x11.window,
                                                                info.info.x11.display,
                                                                0,
                                                                OPTIONS.width,
                                                                OPTIONS.height);
#endif

    RENDER = hylarana_renderer_create(window_handle, RENDER_BACKEND_WGPU);
    MIRROR_SERVICE = new HylaranaService();

    SDL_Event event;
    while (SDL_WaitEvent(&event) == 1)
    {
        if (event.type == SDL_QUIT)
        {
            break;
        }
        else if (event.type == SDL_KEYDOWN)
        {
            switch (event.key.keysym.sym)
            {
            case SDLK_r:
                MIRROR_SERVICE->CreateHylaranaReceiver();

                break;
            case SDLK_s:
                MIRROR_SERVICE->CreateHylaranaSender();

                break;
            case SDLK_k:
                MIRROR_SERVICE->Close();

                break;
            }
        }
    }

    hylarana_renderer_destroy(RENDER);
    hylarana_window_handle_destroy(window_handle);
    hylarana_shutdown();

    SDL_DestroyWindow(window);
    SDL_Quit();

    delete MIRROR_SERVICE;
    return 0;
}

#endif
