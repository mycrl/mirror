//
//  main.cpp
//  sender
//
//  Created by Panda on 2024/4/13.
//

#include <cmdline.h>
#include <string>

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
#include <mirror.h>
}

static Render RENDER = nullptr;

static struct
{
    VideoEncoderType encoder;
    VideoDecoderType decoder;
    std::string server;
    int width;
    int height;
    int fps;
    int id;
} OPTIONS = {};

VideoEncoderType encoder_from_str(std::string value)
{
    if (value == "libx264")
    {
        return VIDEO_ENCODER_X264;
    }
    else if (value == "h264_qsv")
    {
        return VIDEO_ENCODER_QSV;
    }
    else if (value == "h264_nvenc")
    {
        return VIDEO_ENCODER_CUDA;
    }
    else if (value == "h264_videotoolbox")
    {
        return VIDEO_ENCODER_VIDEOTOOLBOX;
    }
    else
    {
        throw std::runtime_error("invalid encoder");
    }
}

VideoDecoderType decoder_from_str(std::string value)
{
    if (value == "h264")
    {
        return VIDEO_DECODER_H264;
    }
    else if (value == "d3d11va")
    {
        return VIDEO_DECODER_D3D11;
    }
    else if (value == "h264_qsv")
    {
        return VIDEO_DECODER_QSV;
    }
    else if (value == "h264_cuvid")
    {
        return VIDEO_DECODER_CUDA;
    }
    else if (value == "h264_videotoolbox")
    {
        return VIDEO_DECODER_VIDEOTOOLBOX;
    }
    else
    {
        throw std::runtime_error("invalid decoder");
    }
}

void cli_parse(std::string cmd)
{
    cmdline::parser args;

#ifdef WIN32
    args.add<std::string>("encoder", '\0', "video encoder", false, "h264_qsv");
    args.add<std::string>("decoder", '\0', "video decoder", false, "d3d11va");
#elif MACOS
    args.add<std::string>("encoder", '\0', "video encoder", false, "h264_videotoolbox");
    args.add<std::string>("decoder", '\0', "video decoder", false, "h264_videotoolbox");
#else
    args.add<std::string>("encoder", '\0', "video encoder", false, "libx264");
    args.add<std::string>("decoder", '\0', "video decoder", false, "h264");
#endif

    args.add<std::string>("server", '\0', "server", false, "192.168.2.88:8088");
    args.add<int>("width", '\0', "video width", false, 1280);
    args.add<int>("height", '\0', "video height", false, 720);
    args.add<int>("fps", '\0', "video frame rate/s", false, 24);
    args.add<int>("id", '\0', "channel number", false, 0);

    if (cmd.length() > 0)
    {
        args.parse_check(cmd);
    }

    OPTIONS.encoder = encoder_from_str(args.get<std::string>("encoder"));
    OPTIONS.decoder = decoder_from_str(args.get<std::string>("decoder"));
    OPTIONS.server = args.get<std::string>("server");
    OPTIONS.width = args.get<int>("width");
    OPTIONS.height = args.get<int>("height");
    OPTIONS.fps = args.get<int>("fps");
    OPTIONS.id = args.get<int>("id");
}

class MirrorService
{
public:
    MirrorService()
    {
        MirrorDescriptor mirror_options;
        mirror_options.server = const_cast<char*>(OPTIONS.server.c_str());
        mirror_options.multicast = const_cast<char*>("239.0.0.1");
        mirror_options.mtu = 1500;

        _mirror = mirror_create(mirror_options);
    }

    ~MirrorService()
    {
        Close();

        if (_mirror != nullptr)
        {
            mirror_destroy(_mirror);
            _mirror = nullptr;
        }
    }

    bool CreateMirrorSender()
    {
        if (_sender != nullptr)
        {
            return true;
        }

        auto video_sources = mirror_get_sources(SOURCE_TYPE_CAMERA);
        auto audio_sources = mirror_get_sources(SOURCE_TYPE_AUDIO);

        VideoDescriptor video_options;
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

        AudioDescriptor audio_options;
        audio_options.encoder.sample_rate = 48000;
        audio_options.encoder.bit_rate = 64000;

        for (int i = 0; i < audio_sources.size; i++)
        {
            if (audio_sources.items[i].is_default)
            {
                audio_options.source = &audio_sources.items[i];
            }
        }

        SenderDescriptor options;
        options.video = &video_options;
        options.audio = nullptr;
        options.multicast = false;

        FrameSink sink;
        sink.video = MirrorService::video_proc;
        sink.audio = MirrorService::audio_proc;
        sink.close = MirrorService::close_proc;
        sink.ctx = (void*)this;

        _sender = mirror_create_sender(_mirror,
                                       OPTIONS.id,
                                       options,
                                       sink);
        if (_sender == nullptr)
        {
            return false;
        }

        _is_runing = true;
        return true;
    }

    bool CreateMirrorReceiver()
    {
        if (_receiver != nullptr)
        {
            return true;
        }

        FrameSink sink;
        sink.video = MirrorService::video_proc;
        sink.audio = MirrorService::audio_proc;
        sink.close = MirrorService::close_proc;
        sink.ctx = (void*)this;

        _receiver = mirror_create_receiver(_mirror,
                                           OPTIONS.id,
                                           OPTIONS.decoder,
                                           sink);
        if (_receiver == nullptr)
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

        if (_sender != nullptr)
        {
            mirror_sender_destroy(_sender);
            _sender = nullptr;
        }

        if (_receiver != nullptr)
        {
            mirror_receiver_destroy(_receiver);
            _receiver = nullptr;
        }
    }
private:
    Mirror _mirror = nullptr;
    Sender _sender = nullptr;
    Receiver _receiver = nullptr;
    bool _is_runing = true;

    static bool video_proc(void* _, VideoFrame* frame)
    {
        return renderer_on_video(RENDER, frame);
    }

    static bool audio_proc(void* _, AudioFrame* frame)
    {
        return renderer_on_audio(RENDER, frame);
    }

    static void close_proc(void* ctx)
    {
        auto mirror = (MirrorService*)ctx;
        mirror->Close();
    }
};

static MirrorService* MIRROR_SERVICE = nullptr;

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
            if (!MIRROR_SERVICE->CreateMirrorSender())
            {
                MessageBox(nullptr,
                           TEXT("Failed to create sender"),
                           TEXT("Error"),
                           0);
            }

            break;
        case 'R':
            if (!MIRROR_SERVICE->CreateMirrorReceiver())
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

int WinMain(HINSTANCE hinstance,
            HINSTANCE _prev_instance,
            LPSTR cmd_line,
            int _show_cmd)
{
    cli_parse(std::string(cmd_line));

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

    auto window_handle = create_window_handle_for_win32(hwnd,
                                                        OPTIONS.width,
                                                        OPTIONS.height);
    RENDER = renderer_create(window_handle, RENDER_BACKEND_WGPU);
    MIRROR_SERVICE = new MirrorService();

    MSG message;
    while (GetMessage(&message, nullptr, 0, 0))
    {
        TranslateMessage(&message);
        DispatchMessage(&message);
    }

    renderer_destroy(RENDER);
    window_handle_destroy(window_handle);
    DestroyWindow(hwnd);

    delete MIRROR_SERVICE;
    return 0;
}

#else

int main(int argc, char* argv[])
{
    cli_parse(argc >= 2 ? std::string(argv[1]) : "");
    mirror_startup();

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
    auto window_handle = create_window_handle_for_appkit(ns_view,
                                                         OPTIONS.width,
                                                         OPTIONS.height);
#endif

#ifdef LINUX
    auto window_handle = create_window_handle_for_xlib(info.info.x11.window,
                                                       info.info.x11.display,
                                                       OPTIONS.width,
                                                       OPTIONS.height);
#endif

    RENDER = renderer_create(window_handle, RENDER_BACKEND_WGPU);
    MIRROR_SERVICE = new MirrorService();

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
                MIRROR_SERVICE->CreateMirrorReceiver();

                break;
            case SDLK_s:
                MIRROR_SERVICE->CreateMirrorSender();

                break;
            case SDLK_k:
                MIRROR_SERVICE->Close();

                break;
            }
        }
    }

    renderer_destroy(RENDER);
    window_handle_destroy(window_handle);
    mirror_shutdown();

    SDL_DestroyWindow(window);
    SDL_Quit();

    delete MIRROR_SERVICE;
    return 0;
}

#endif
