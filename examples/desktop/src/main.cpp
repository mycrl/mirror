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

static std::string BIND = std::string("0.0.0.0:2300");
static int MTU = 1500;

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

    mirror_init();

    SDL_Rect sdl_rect;
    sdl_rect.x = 0;
    sdl_rect.y = 0;
    sdl_rect.w = 1280;
    sdl_rect.h = 720;

    DeviceManagerOptions options;
    options.device.width = sdl_rect.w;
    options.device.height = sdl_rect.h;
    options.device.fps = 30;
    options.video_encoder.codec_name = const_cast<char*>("libx264");
    options.video_encoder.width = sdl_rect.w;
    options.video_encoder.height = sdl_rect.h;
    options.video_encoder.frame_rate = 30;
    options.video_encoder.bit_rate = 500 * 1024 * 8;
    options.video_encoder.max_b_frames = 0;
    options.video_encoder.key_frame_interval = 10;
    
    mirror::DeviceManagerService* device_manager = new mirror::DeviceManagerService(options);
    mirror::MirrorService* mirror = new mirror::MirrorService("239.0.0.1");
    
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
                                                 SDL_PIXELFORMAT_IYUV,
                                                 SDL_TEXTUREACCESS_STREAMING,
                                                 sdl_rect.w,
                                                 sdl_rect.h);
    
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

            // R: 21, S: 22
            if (event.key.keysym.scancode == 22)
            {
                auto devices = device_manager->GetDevices();
                device_manager->SetInputDevice(devices.device_list[0]);
                created = mirror->CreateSender(device_manager, MTU, BIND);
            }
            else if (event.key.keysym.scancode == 21)
            {
                std::string decoder = std::string("h264");
                created = mirror->CreateReceiver(BIND, decoder, [&](void* _, VideoFrame* frame) {
                    SDL_UpdateNVTexture(sdl_texture,
                                        &sdl_rect,
                                        frame->data[0],
                                        frame->linesize[0],
                                        frame->data[1],
                                        frame->linesize[1]);
                    SDL_RenderClear(sdl_renderer);
                    SDL_RenderCopy(sdl_renderer, sdl_texture, nullptr, &sdl_rect);
                    SDL_RenderPresent(sdl_renderer);
                    
                    return true;
                }, nullptr);
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
