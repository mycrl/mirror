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
    
    // B1. 初始化SDL子系统：缺省(事件处理、文件IO、线程)、视频、音频、定时器
    if (SDL_Init(SDL_INIT_VIDEO | SDL_INIT_AUDIO | SDL_INIT_TIMER))
    {
        return -1;
    }
    
    // B2. 创建SDL窗口，SDL 2.0支持多窗口
    //     SDL_Window即运行程序后弹出的视频窗口，同SDL 1.x中的SDL_Surface
    SDL_Window* screen = SDL_CreateWindow("simple",
                                          SDL_WINDOWPOS_UNDEFINED,// 不关心窗口X坐标
                                          SDL_WINDOWPOS_UNDEFINED,// 不关心窗口Y坐标
                                          sdl_rect.w,
                                          sdl_rect.h,
                                          SDL_WINDOW_OPENGL);
    if (screen == NULL)
    {
        return -2;
    }
    
    // B3. 创建SDL_Renderer
    //     SDL_Renderer：渲染器
    SDL_Renderer* sdl_renderer = SDL_CreateRenderer(screen, -1, 0);
    
    // B4. 创建SDL_Texture
    //     一个SDL_Texture对应一帧YUV数据，同SDL 1.x中的SDL_Overlay
    //     此处第2个参数使用的是SDL中的像素格式，对比参考注释A7
    //     FFmpeg中的像素格式AV_PIX_FMT_YUV420P对应SDL中的像素格式SDL_PIXELFORMAT_IYUV
    SDL_Texture* sdl_texture = SDL_CreateTexture(sdl_renderer,
                                                 SDL_PIXELFORMAT_IYUV,
                                                 SDL_TEXTUREACCESS_STREAMING,
                                                 sdl_rect.w,
                                                 sdl_rect.h);
    
    mirror->CreateReceiver(BIND, std::string("libx1264"), [&](void* _, VideoFrame* frame) {
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
    
    bool created = false;
    SDL_Event event;
    while (SDL_PollEvent(&event))
    {
        if (event.type == SDL_KEYDOWN && event.key.keysym.scancode == SDL_SCANCODE_KP_ENTER)
        {
            if (created)
            {
                continue;
            }
            
            auto devices = device_manager->GetDevices();
            device_manager->SetInputDevice(devices.device_list[0]);
            if (!mirror->CreateSender(device_manager, MTU, BIND)) {
                break;
            }
            else
            {
                created = true;
            }
        }
    }
    
    SDL_Quit();
    return 0;
}
