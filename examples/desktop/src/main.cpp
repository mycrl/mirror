//
//  main.cpp
//  example
//
//  Created by Panda on 2024/4/13.
//

#include <thread>

#ifdef WIN32
#include <windows.h>
#endif

#include <mirror.h>
#include <SDL.h>
#include <SDL_video.h>
#include <SDL_render.h>
#include <SDL_rect.h>

#ifdef WIN32
int WinMain(HINSTANCE _instance,
            HINSTANCE _prev_instance,
            LPSTR _cmd_line,
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

    SDL_Rect sdl_rect;
    sdl_rect.x = 0;
    sdl_rect.y = 0;
    sdl_rect.w = 1280;
    sdl_rect.h = 720;

    MirrorOptions options;
    options.video.encoder = const_cast<char*>("h264_qsv");
    options.video.decoder = const_cast<char*>("h264");
    options.video.width = sdl_rect.w;
    options.video.height = sdl_rect.h;
    options.video.frame_rate = 30;
    options.video.bit_rate = 500 * 1024 * 8;
    options.video.max_b_frames = 0;
    options.video.key_frame_interval = 10;
    options.multicast = const_cast<char*>("239.0.0.1");
    options.mtu = 1500;
    mirror::Init(options);
    
    if (SDL_Init(SDL_INIT_VIDEO | SDL_INIT_AUDIO | SDL_INIT_TIMER))
    {
        return -1;
    }
    
    SDL_Window* screen = SDL_CreateWindow("simple",
                                          SDL_WINDOWPOS_UNDEFINED,
                                          SDL_WINDOWPOS_UNDEFINED,
                                          sdl_rect.w,
                                          sdl_rect.h,
                                          SDL_WINDOW_OPENGL);
    if (screen == NULL)
    {
        return -2;
    }
    
    SDL_Renderer* sdl_renderer = SDL_CreateRenderer(screen, -1, 0);
    SDL_Texture* sdl_texture = SDL_CreateTexture(sdl_renderer,
                                                 SDL_PIXELFORMAT_NV12,
                                                 SDL_TEXTUREACCESS_STREAMING,
                                                 sdl_rect.w,
                                                 sdl_rect.h);
    
    mirror::MirrorService* mirror = new mirror::MirrorService();
    bool created = false;
    SDL_Event event;

    while (SDL_WaitEvent(&event))
    {
        if (event.type == SDL_KEYDOWN)
        {
            if (created)
            {
                continue;
            }

            switch (event.key.keysym.scancode)
            {
            case SDL_SCANCODE_S:
            {
                auto devices = mirror::DeviceManagerService::GetDevices(DeviceKind::Video);
                mirror::DeviceManagerService::SetInputDevice(devices.device_list[0]);

                std::string bind = "0.0.0.0:3200";
                created = mirror->CreateSender(bind).has_value();

                break;
            }
            case SDL_SCANCODE_R:
            {
                std::string bind = "0.0.0.0:3200";
                created = mirror->CreateReceiver(bind, [&](void* _, VideoFrame* frame) {
                    if (SDL_UpdateNVTexture(sdl_texture,
                                        &sdl_rect,
                                        frame->data[0],
                                        frame->linesize[0],
                                        frame->data[1],
                                        frame->linesize[1]) == 0)
                    {
                        if (SDL_RenderClear(sdl_renderer) == 0)
                        {
                            if (SDL_RenderCopy(sdl_renderer, sdl_texture, nullptr, &sdl_rect) == 0)
                            {
                                SDL_RenderPresent(sdl_renderer);
                                return true;
                            }
                        }                 
                    }

                    return false;
                }, nullptr).has_value();

                break;
            }
            default:
                break;
            }
        }
        else if (event.type == SDL_QUIT)
        {
            break;
        }
    }

    SDL_Quit();
    return 0;
}
