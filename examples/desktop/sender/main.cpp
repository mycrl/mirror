//
//  main.cpp
//  sender
//
//  Created by Panda on 2024/4/13.
//

#ifdef WIN32
#include <windows.h>
#endif

#include <SDL.h>
#include <SDL_video.h>

#include "../common.h"

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

    SDL_Rect sdl_rect;
    sdl_rect.x = 0;
    sdl_rect.y = 0;
    sdl_rect.w = args.ArgsParams.width;
    sdl_rect.h = args.ArgsParams.height;

    MirrorOptions options;
    options.video.encoder = const_cast<char*>(args.ArgsParams.encoder.c_str());
    options.video.decoder = const_cast<char*>(args.ArgsParams.decoder.c_str());
    options.video.width = sdl_rect.w;
    options.video.height = sdl_rect.h;
    options.video.frame_rate = args.ArgsParams.fps;
    options.video.key_frame_interval = args.ArgsParams.fps;
    options.video.bit_rate = 500 * 1024 * 8;
    options.audio.sample_rate = 48000;
    options.audio.bit_rate = 64000;
    options.server = const_cast<char*>(args.ArgsParams.server.c_str());
    options.multicast = const_cast<char*>("239.0.0.1");
    options.mtu = 1400;
    mirror::Init(options);

    if (SDL_Init(SDL_INIT_VIDEO | SDL_INIT_TIMER))
    {
        return -1;
    }

    SDL_Window* screen = SDL_CreateWindow("sender",
                                          SDL_WINDOWPOS_UNDEFINED,
                                          SDL_WINDOWPOS_UNDEFINED,
                                          sdl_rect.w,
                                          sdl_rect.h,
                                          SDL_WINDOW_OPENGL | SDL_WINDOW_MAXIMIZED);
    if (screen == NULL)
    {
        return -2;
    }

    SDL_Renderer* sdl_renderer = SDL_CreateRenderer(screen, -1, SDL_RENDERER_ACCELERATED);
    SDL_Texture* sdl_texture = SDL_CreateTexture(sdl_renderer,
                                                 SDL_PIXELFORMAT_NV12,
                                                 SDL_TEXTUREACCESS_STREAMING,
                                                 sdl_rect.w,
                                                 sdl_rect.h);

    mirror::MirrorService* mirror = new mirror::MirrorService();

    auto devices = mirror::DeviceManagerService::GetDevices(DeviceKind::Screen);
    if (devices.device_list.size() == 0)
    {
        MessageBox(nullptr, TEXT("Not found a device!"), TEXT("Error"), 0);
        return -10;
    }
    else
    {
        mirror::DeviceManagerService::SetInputDevice(devices.device_list[0]);
    }

    std::optional<mirror::MirrorService::MirrorSender> sender = std::nullopt;
    Render* render = new Render(&sdl_rect, sdl_texture, sdl_renderer, false, [&]
                                {
                                    sender = std::nullopt;
                                    MessageBox(nullptr, TEXT("sender is closed!"), TEXT("Info"), 0);
                                });

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
                case SDLK_t:
                    if (sender.has_value())
                    {
                        sender.value().SetMulticast(!sender.value().GetMulticast());
                    }

                    break;
                case SDLK_s:
                    if (!sender.has_value())
                    {
                        sender = mirror->CreateSender(args.ArgsParams.id, render);
                        if (!sender.has_value())
                        {
                            MessageBox(nullptr, TEXT("Failed to create sender!"), TEXT("Error"), 0);
                            SDL_Quit();
                            mirror::Quit();

                            return -1;
                        }
                    }

                    break;
                case SDLK_k:
                    if (sender.has_value())
                    {
                        sender.value().Close();
                        sender = std::nullopt;
                    }

                    break;
            }
        }
    }

    SDL_Quit();
    mirror::Quit();
    return 0;
}
