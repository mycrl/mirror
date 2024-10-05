#ifndef ARGS_H
#define ARGS_H
#pragma once

extern "C"
{
#include <mirror.h>
}

#include <string>
#include <vector>

class Args
{
public:
    struct Params
    {
        VideoEncoderType encoder = VIDEO_ENCODER_QSV;
#ifdef WIN32
        VideoDecoderType decoder = VIDEO_DECODER_D3D11;
#elif LINUX
        VideoDecoderType decoder = VIDEO_DECODER_VAAPI;
#endif
        std::string server = "127.0.0.1:8080";
        int width = 1280;
        int height = 720;
        int fps = 24;
        int id = 0;
    };

    Args(std::string args);

    struct Params ArgsParams;
private:
    std::vector<std::string> finds(std::string input, std::string delimiter);
};

#endif
