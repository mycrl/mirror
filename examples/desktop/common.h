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

#include <mirror.h>
#include <SDL_render.h>
#include <SDL_rect.h>

class Render : public mirror::MirrorService::AVFrameSink
{
public:
	Render(SDL_Rect* sdl_rect,
		   SDL_Texture* sdl_texture,
		   SDL_Renderer* sdl_renderer)
		: _sdl_rect(sdl_rect)
		, _sdl_texture(sdl_texture)
		, _sdl_renderer(sdl_renderer)
	{}

	bool OnVideoFrame(struct VideoFrame* frame)
	{
		if (SDL_UpdateNVTexture(_sdl_texture,
								_sdl_rect,
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
private:
	SDL_Rect* _sdl_rect;
	SDL_Texture* _sdl_texture;
	SDL_Renderer* _sdl_renderer;
};

class Args
{
public:
	struct Params
	{
        int fps = 30;
		int width = 1280;
		int height = 720;
		std::string bind = "0.0.0.0:8080";
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

            if (kv[0] == "fps")
			{
				ArgsParams.fps = std::stoi(kv[1]);
			}
			if (kv[0] == "width")
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
			else if (kv[0] == "bind")
			{
				ArgsParams.bind = kv[1];
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
