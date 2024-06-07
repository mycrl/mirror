//
//  common.h
//  examples
//
//  Created by Panda on 2024/5/15.
//

#ifndef EXMPLES_COMMON_H
#define EXMPLES_COMMON_H
#pragma once

#include <windows.h>

#include <string>
#include <vector>
#include <functional>

#include <mirror.h>
#include <SDL_render.h>
#include <SDL_rect.h>

class Render : public mirror::MirrorService::AVFrameSink
{
public:
	Render(SDL_Rect* sdl_rect,
		   SDL_Window* screen,
		   bool is_render,
		   std::function<void()> closed_callback)
		: _is_render(is_render)
		, _closed_callback(closed_callback)
		, _sdl_rect(sdl_rect)
		, _screen(screen)
	{}

	bool OnVideoFrame(struct VideoFrame* frame)
	{
		if (!_is_render)
		{
			return true;
		}

		if (_sdl_renderer == nullptr)
		{
			_sdl_renderer = SDL_CreateRenderer(_screen, -1, SDL_RENDERER_ACCELERATED);
		}

		if (_sdl_texture == nullptr)
		{
			_sdl_texture = SDL_CreateTexture(_sdl_renderer,
											 SDL_PIXELFORMAT_NV12,
											 SDL_TEXTUREACCESS_STREAMING,
											 frame->rect.width,
											 frame->rect.height);
		}

		SDL_Rect sdl_rect;
		sdl_rect.w = frame->rect.width;
		sdl_rect.h = frame->rect.height;
		sdl_rect.x = 0;
		sdl_rect.y = 0;

		if (SDL_UpdateNVTexture(_sdl_texture,
								&sdl_rect,
								frame->data[0],
								frame->linesize[0],
								frame->data[1],
								frame->linesize[1]) == 0)
		{
			if (SDL_RenderClear(_sdl_renderer) == 0)
			{
				if (SDL_RenderCopy(_sdl_renderer, _sdl_texture, nullptr, _sdl_rect) == 0)
				{
					SDL_RenderPresent(_sdl_renderer);
					return true;
				}
			}
		}

		return false;
	}

	bool OnAudioFrame(struct AudioFrame* frame)
	{
		return true;
	}

	void OnClose()
	{
		_closed_callback();
	}
private:
	bool _is_render;
	SDL_Window* _screen = nullptr;
	SDL_Rect* _sdl_rect = nullptr;
	SDL_Texture* _sdl_texture = nullptr;
	SDL_Renderer* _sdl_renderer = nullptr;
	std::function<void()> _closed_callback;
};

class Args
{
public:
	struct Params
	{
		int id = 0;
        int fps = 24;
		int width = 1280;
		int height = 720;
		std::string server = "127.0.0.1:8080";
		std::string encoder = mirror_find_video_encoder();
		std::string decoder = mirror_find_video_decoder();
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
			} else if (kv[0] == "width")
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

#endif // EXMPLES_COMMON_H
