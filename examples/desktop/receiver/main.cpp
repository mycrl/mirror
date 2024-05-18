//
//  main.cpp
//  receiver
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
	options.video.max_b_frames = 0;
    options.audio.sample_rate = 48000;
    options.audio.bit_rate = 6000;
	options.multicast = const_cast<char*>("239.0.0.1");
	options.mtu = 1400;
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

    Render* render = new Render(&sdl_rect, sdl_texture, sdl_renderer);
	mirror::MirrorService* mirror = new mirror::MirrorService();

	auto receiver = mirror->CreateReceiver(args.ArgsParams.bind, render);
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

    if (receiver.has_value())
	{
        receiver.value().Close();
	}

	SDL_Quit();
    mirror::Quit();

	return 0;
}
