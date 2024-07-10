//
//  main.cpp
//  sender
//
//  Created by Panda on 2024/4/13.
//

#ifdef WIN32
#include <windows.h>
#endif

#define SDL_MAIN_HANDLED
#include <mirror.h>
#include <mutex>
#include <string>
#include <vector>
#include <thread>
#include <functional>

class Args
{
public:
    struct Params
    {
        std::string encoder = mirror_find_video_encoder();
        std::string decoder = mirror_find_video_decoder();
        std::string server = "127.0.0.1:8080";
        int width = 1280;
        int height = 720;
        int fps = 30;
        int id = 0;
    };

    Args(std::string args)
    {
        for (auto path : finds(args, ","))
        {
            auto kv = finds(path, "=");
            if (kv.size() < 2)
            {
                continue;
            }

            if (kv[0] == "id")
            {
                ArgsParams.id = std::stoi(kv[1]);
            }
            else if (kv[0] == "fps")
            {
                ArgsParams.fps = std::stoi(kv[1]);
            }
            else if (kv[0] == "width")
            {
                ArgsParams.width = std::stoi(kv[1]);
            }
            else if (kv[0] == "height")
            {
                ArgsParams.height = std::stoi(kv[1]);
            }
            else if (kv[0] == "encoder")
            {
                ArgsParams.encoder = kv[1];
            }
            else if (kv[0] == "decoder")
            {
                ArgsParams.decoder = kv[1];
            }
            else if (kv[0] == "server")
            {
                ArgsParams.server = kv[1];
            }
        }
    }

    struct Params ArgsParams;
private:
    std::vector<std::string> finds(std::string input, std::string delimiter)
    {
        size_t iter = 0;
        std::vector<std::string> tokens;
        while (iter < input.size())
        {
            iter = input.find(delimiter);
            tokens.push_back(input.substr(0, iter));
            input.erase(0, iter + delimiter.length());
        }

        if (input.size() > 0)
        {
            tokens.push_back(input);
        }

        return tokens;
    }
};

class SimpleRender : public mirror::MirrorService::AVFrameSink
{
public:
    SimpleRender(Args& args,
                 HWND hwnd,
                 HINSTANCE hinstance,
                 std::function<void()> closed_callback)
        : _callback(closed_callback)
        , _args(args)
    {
        Size size;
        size.width = args.ArgsParams.width;
        size.height = args.ArgsParams.height;

        _window_handle = mirror_create_window_handle(hwnd, hinstance);
        _render = mirror_create_render(size, size, _window_handle);
        if (_render == nullptr)
        {
            MessageBox(nullptr, TEXT("failed to create render!"), TEXT("Error"), 0);
        }
    }

    ~SimpleRender()
    {
        mirror_render_destroy(_render);
        mirror_window_handle_destroy(_window_handle);
        _runing = false;
    }

    void SetTitle(std::string title)
    {
        std::string base = "example - s/create sender, r/create receiver, k/stop";
        if (title.length() > 0)
        {
            base += " - [";
            base += title;
            base += "]";
        }
    }

    bool OnVideoFrame(struct VideoFrame* frame)
    {
        return mirror_render_on_video(_render, frame);
    }

    bool OnAudioFrame(struct AudioFrame* frame)
    {
        return true;
    }

    void OnClose()
    {
        _callback();
        SetTitle("");
        Clear();
    }

    void Clear()
    {

    }

    bool IsRender = true;
private:
    Args& _args;
    bool _runing = true;
    Render _render = nullptr;
    WindowHandle _window_handle = nullptr;
    std::function<void()> _callback;
    std::mutex _mutex;
};

class MirrorImplementation
{
public:
    MirrorImplementation(Args& args,
                         HWND hwnd,
                         HINSTANCE hinstance) : _args(args)
    {
        MirrorOptions options;
        options.video.encoder = const_cast<char*>(args.ArgsParams.encoder.c_str());
        options.video.decoder = const_cast<char*>(args.ArgsParams.decoder.c_str());
        options.video.width = args.ArgsParams.width;
        options.video.height = args.ArgsParams.height;
        options.video.frame_rate = args.ArgsParams.fps;
        options.video.key_frame_interval = 21;
        options.video.bit_rate = 500 * 1024 * 8;
        options.audio.sample_rate = 48000;
        options.audio.bit_rate = 64000;
        options.server = const_cast<char*>(args.ArgsParams.server.c_str());
        options.multicast = const_cast<char*>("239.0.0.1");
        options.mtu = 1400;
        mirror::Init(options);

        _mirror = new mirror::MirrorService();
        _render = new SimpleRender(args,
                                   hwnd,
                                   hinstance,
                                   [&]
                                   {
                                       _sender = std::nullopt;
                                       _receiver = std::nullopt;
                                       MessageBox(nullptr, TEXT("sender/receiver is closed!"), TEXT("Info"), 0);
                                   });
    }

    ~MirrorImplementation()
    {
        delete _mirror;
        delete _render;
        mirror::Quit();
    }

    bool CreateMirrorSender()
    {
        if (_sender.has_value())
        {
            return true;
        }
        else
        {
            _render->IsRender = false;
        }

        mirror::DeviceManagerService::Start();
        auto devices = mirror::DeviceManagerService::GetDevices(DeviceKind::Screen);
        if (devices.device_list.size() == 0)
        {
            return false;
        }

        CaptureSettings settings;
        settings.method = CaptureMethod::WGC;

        mirror::DeviceManagerService::SetInputDevice(devices.device_list[0], &settings);
        _sender = _mirror->CreateSender(_args.ArgsParams.id, _render);
        if (!_sender.has_value())
        {
            return false;
        }

        _render->SetTitle("sender");
        return true;
    }

    bool CreateMirrorReceiver()
    {
        if (_receiver.has_value())
        {
            return true;
        }
        else
        {
            _render->IsRender = true;
        }

        _receiver = _mirror->CreateReceiver(_args.ArgsParams.id, _render);
        if (!_receiver.has_value())
        {
            return false;
        }

        _render->SetTitle("receiver");
        return true;
    }

    void Close()
    {
        if (_sender.has_value())
        {
            _sender.value().Close();
            _sender = std::nullopt;
            mirror::DeviceManagerService::Stop();
        }

        if (_receiver.has_value())
        {
            _receiver.value().Close();
            _receiver = std::nullopt;
        }

        _render->SetTitle("");
        _render->Clear();
    }
private:
    Args& _args;
    SimpleRender* _render = nullptr;
    mirror::MirrorService* _mirror = nullptr;
    std::optional<mirror::MirrorService::MirrorSender> _sender = std::nullopt;
    std::optional<mirror::MirrorService::MirrorReceiver> _receiver = std::nullopt;
};

static MirrorImplementation* mirror_impl = nullptr;

#ifdef WIN32
LRESULT CALLBACK window_handle_proc(HWND hwnd, UINT message, WPARAM wparam, LPARAM lparam)
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
                    if (!mirror_impl->CreateMirrorSender())
                    {
                        MessageBox(nullptr, TEXT("Failed to create sender"), TEXT("Error"), 0);
                    }

                    break;
                case 'R':
                    if (!mirror_impl->CreateMirrorReceiver())
                    {
                        MessageBox(nullptr, TEXT("Failed to create receiver"), TEXT("Error"), 0);
                    }

                    break;
                case 'K':
                    mirror_impl->Close();
                    break;
                default:
                    break;
            }

            return 0;
        default:
            return DefWindowProc(hwnd, message, wparam, lparam);
    }
}
#endif // WIN32

#ifdef WIN32
int WinMain(HINSTANCE hinstance,
            HINSTANCE _prev_instance,
            LPSTR cmd_line,
            int _show_cmd)
#else
int main()
#endif // WIN32
{

#ifdef WIN32
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
    wc.lpszClassName = "GLSample";

    RegisterClass(&wc);

    Args args = Args(std::string(cmd_line));
    HWND hwnd = CreateWindow("GLSample",
                             "OpenGL Window",
                             WS_CAPTION | WS_POPUPWINDOW | WS_VISIBLE,
                             0,
                             0,
                             args.ArgsParams.width,
                             args.ArgsParams.height,
                             nullptr,
                             nullptr,
                             hinstance,
                             nullptr);
    mirror_impl = new MirrorImplementation(args, hwnd, hinstance);

    MSG message;
    while (GetMessage(&message, nullptr, 0, 0))
    {
        TranslateMessage(&message);
        DispatchMessage(&message);
    }

    DestroyWindow(hwnd);
#endif // WIN32

    delete mirror_impl;
    return 0;
}
