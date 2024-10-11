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
#ifdef WIN32
        VideoEncoderType encoder = VIDEO_ENCODER_QSV;
        VideoDecoderType decoder = VIDEO_DECODER_D3D11;
#else
        VideoEncoderType encoder = VIDEO_ENCODER_X264;
        VideoDecoderType decoder = VIDEO_DECODER_H264;
#endif
        std::string server = "192.168.2.88:8088";
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
