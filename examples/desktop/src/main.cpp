//
//  main.cpp
//  example
//
//  Created by Panda on 2024/4/13.
//

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
    SDL_Rect sdl_rect;
    sdl_rect.x = 0;
    sdl_rect.y = 0;
    sdl_rect.w = 1920;
    sdl_rect.h = 1080;

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

    SDL_UpdateYUVTexture(sdl_texture,                   // sdl texture
                            &sdl_rect,                     // sdl rect
                            p_frm_yuv->data[0],            // y plane
                            p_frm_yuv->linesize[0],        // y pitch
                            p_frm_yuv->data[1],            // u plane
                            p_frm_yuv->linesize[1],        // u pitch
                            p_frm_yuv->data[2],            // v plane
                            p_frm_yuv->linesize[2]         // v pitch
                            );
    
    // B6. 使用特定颜色清空当前渲染目标
    SDL_RenderClear(sdl_renderer);
    // B7. 使用部分图像数据(texture)更新当前渲染目标
    SDL_RenderCopy(sdl_renderer,                        // sdl renderer
                    sdl_texture,                         // sdl texture
                    NULL,                                // src rect, if NULL copy texture
                    &sdl_rect                            // dst rect
                    );
    // B8. 执行渲染，更新屏幕显示
    SDL_RenderPresent(sdl_renderer); 

    
    SDL_Quit();
    return 0;
}
