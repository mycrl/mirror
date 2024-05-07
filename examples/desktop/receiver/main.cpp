//
//  main.cpp
//  receiver
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

class Render: public mirror::MirrorService::AVFrameSink 
{
public:
    Render(SDL_Rect* sdl_rect,
           SDL_Texture* sdl_texture,
           SDL_Renderer* sdl_renderer)
        : _sdl_rect(sdl_rect)
        , _sdl_texture(sdl_texture)
        , _sdl_renderer(sdl_renderer)
    {
    }

    bool OnVideoFrame(struct VideoFrame* frame)
    {
        // if (SDL_UpdateNVTexture(_sdl_texture,
        // _sdl_rect,
        // frame->data[0],
        // frame->linesize[0],
        // frame->data[1],
        // frame->linesize[1]) == 0)
        // {
        //     if (SDL_RenderClear(_sdl_renderer) == 0)
        //     {
        //         if (SDL_RenderCopy(_sdl_renderer, _sdl_texture, nullptr, _sdl_rect) == 0)
        //         {
        //             SDL_RenderPresent(_sdl_renderer);
        //             return true;
        //         }
        //     }
        // }

        // return false;
        return true;
    }

    bool OnAudioFrame(struct AudioFrame* frame)
    {
        return true;
    }
private:
    SDL_Rect* _sdl_rect;
    SDL_Texture* _sdl_texture;
    SDL_Renderer* _sdl_renderer;
};

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
	options.video.encoder = const_cast<char*>(mirror_find_video_encoder());
	options.video.decoder = const_cast<char*>(mirror_find_video_decoder());
	options.video.width = sdl_rect.w;
	options.video.height = sdl_rect.h;
	options.video.frame_rate = 30;
	options.video.bit_rate = 500 * 1024 * 8;
	options.video.max_b_frames = 0;
	options.video.key_frame_interval = 15;
    options.audio.sample_rate = 48000;
    options.audio.bit_rate = 6000;
	options.multicast = const_cast<char*>("239.0.0.1");
	options.mtu = 1500;
	mirror::Init(options);

	if (SDL_Init(SDL_INIT_VIDEO | SDL_INIT_AUDIO | SDL_INIT_TIMER))
	{
		return -1;
	}

	SDL_Window* screen = SDL_CreateWindow("receiver",
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

    Render* render = new Render(&sdl_rect, sdl_texture, sdl_renderer);
	mirror::MirrorService* mirror = new mirror::MirrorService();

	std::string bind = "0.0.0.0:3200";
	auto receiver = mirror->CreateReceiver(bind, render);
	if (!receiver.has_value())
	{
		MessageBox(nullptr, TEXT("Failed to create receiver!"), TEXT("Error"), 0);
		SDL_Quit();
    	mirror::Quit();
		return -1;
	}

	SDL_Event event;
	while (SDL_WaitEvent(&event))
	{
		if (event.type == SDL_QUIT)
		{
			break;
		}
	}

	SDL_Quit();
    mirror::Quit();
	return 0;
}
