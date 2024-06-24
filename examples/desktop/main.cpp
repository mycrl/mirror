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
#include <SDL.h>
#include <SDL_video.h>
#include <SDL_render.h>
#include <SDL_rect.h>
#include <windows.h>
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
        int fps = 24;
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

class Render : public mirror::MirrorService::AVFrameSink
{
public:
    Render(Args& args,
           std::function<void()> closed_callback)
        : _callback(closed_callback)
        , _args(args)
    {

        _audio_spec.freq = 48000;
        _audio_spec.channels = 1;
        _audio_spec.silence = 0;
        _audio_spec.samples = 960;
        _audio_spec.size = 960 * 4;
        _audio_spec.format = AUDIO_S16;
        _audio_spec.callback = nullptr;

        SDL_Init(SDL_INIT_VIDEO | SDL_INIT_AUDIO | SDL_INIT_TIMER);

        _audio = SDL_OpenAudioDevice(SDL_GetAudioDeviceName(3, 0), 
                                     0, 
                                     &_audio_spec, 
                                     nullptr, 
                                     SDL_AUDIO_ALLOW_FREQUENCY_CHANGE);
        SDL_PauseAudioDevice(_audio, 0);

        _window = SDL_CreateWindow("example - s/create sender, r/create receiver, k/stop",
                                   SDL_WINDOWPOS_UNDEFINED,
                                   SDL_WINDOWPOS_UNDEFINED,
                                   args.ArgsParams.width,
                                   args.ArgsParams.height,
                                   SDL_WINDOW_RESIZABLE);

        _renderer = SDL_CreateRenderer(_window, -1, SDL_RENDERER_ACCELERATED);
        _texture = SDL_CreateTexture(_renderer,
                                     SDL_PIXELFORMAT_NV12,
                                     SDL_TEXTUREACCESS_STREAMING,
                                     args.ArgsParams.width,
                                     args.ArgsParams.height);
        std::thread(
            [&]()
            {
                while (_runing)
                {
                    {
                        std::lock_guard<std::mutex> guard(_mutex);
                        if (SDL_RenderClear(_renderer) == 0)
                        {
                            if (SDL_RenderCopy(_renderer, _texture, nullptr, nullptr) == 0)
                            {
                                SDL_RenderPresent(_renderer);
                            }
                        }
                    }

                    SDL_Delay(1000 / 30);
                }

                SDL_Quit();
            }).detach();
    }

    ~Render()
    {
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

        SDL_SetWindowTitle(_window, base.c_str());
    }

    bool OnVideoFrame(struct VideoFrame* frame)
    {
        if (!IsRender)
        {
            return true;
        }

        std::lock_guard<std::mutex> guard(_mutex);
        SDL_UpdateNVTexture(_texture,
                            nullptr,
                            frame->data[0],
                            frame->linesize[0],
                            frame->data[1],
                            frame->linesize[1]);
        return true;
    }

    bool OnAudioFrame(struct AudioFrame* frame)
    {
        if (!IsRender)
        {
            return true;
        }

        return SDL_QueueAudio(_audio, frame->data, frame->frames * 2) == 0;
    }

    void OnClose()
    {
        _callback();
        SetTitle("");
        Clear();
    }

    void Clear()
    {
        std::lock_guard<std::mutex> guard(_mutex);

        size_t size = _args.ArgsParams.width * _args.ArgsParams.height;
        uint8_t* buf = new uint8_t[size];

        SDL_UpdateNVTexture(_texture,
                            nullptr,
                            buf,
                            _args.ArgsParams.width,
                            buf,
                            _args.ArgsParams.width);

        delete buf;
    }

    bool IsRender = true;
private:
    Args& _args;
    bool _runing = true;
    SDL_AudioDeviceID _audio;
    SDL_AudioSpec _audio_spec = { 0 };
    SDL_Window* _window = nullptr;
    SDL_Texture* _texture = nullptr;
    SDL_Renderer* _renderer = nullptr;
    std::function<void()> _callback;
    std::mutex _mutex;
};

class MirrorImplementation
{
public:
    MirrorImplementation(Args& args) : _args(args)
    {
        MirrorOptions options;
        options.video.encoder = const_cast<char*>(args.ArgsParams.encoder.c_str());
        options.video.decoder = const_cast<char*>(args.ArgsParams.decoder.c_str());
        options.video.width = args.ArgsParams.width;
        options.video.height = args.ArgsParams.height;
        options.video.frame_rate = args.ArgsParams.fps;
        options.video.key_frame_interval = args.ArgsParams.fps;
        options.video.bit_rate = 500 * 1024 * 8;
        options.audio.sample_rate = 48000;
        options.audio.bit_rate = 64000;
        options.server = const_cast<char*>(args.ArgsParams.server.c_str());
        options.multicast = const_cast<char*>("239.0.0.1");
        options.mtu = 1400;
        mirror::Init(options);

        _mirror = new mirror::MirrorService();
        _render = new Render(args,
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

        mirror::DeviceManagerService::SetInputDevice(devices.device_list[0]);
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
    Render* _render = nullptr;
    mirror::MirrorService* _mirror = nullptr;
    std::optional<mirror::MirrorService::MirrorSender> _sender = std::nullopt;
    std::optional<mirror::MirrorService::MirrorReceiver> _receiver = std::nullopt;
};

#ifdef WIN32
int WinMain(HINSTANCE _instance,
            HINSTANCE _prev_instance,
            LPSTR cmd_line,
            int _show_cmd)
#else
int main()
#endif
{

#ifdef WIN32
    AttachConsole(ATTACH_PARENT_PROCESS);
    freopen("CONIN$", "r+t", stdin);
    freopen("CONOUT$", "w+t", stdout);
#endif

    Args args = Args(std::string(cmd_line));
    MirrorImplementation mirror(args);

    SDL_Event event;
    while (SDL_WaitEvent(&event))
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
                    if (!mirror.CreateMirrorReceiver())
                    {
                        MessageBox(nullptr, TEXT("Failed to create receiver"), TEXT("Error"), 0);
                    }

                    break;
                case SDLK_s:
                    if (!mirror.CreateMirrorSender())
                    {
                        MessageBox(nullptr, TEXT("Failed to create sender"), TEXT("Error"), 0);
                    }

                    break;
                case SDLK_k:
                    mirror.Close();

                    break;
            }
        }
    }

    return 0;
}
