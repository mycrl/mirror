#include "./args.h"

Args::Args(std::string args)
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
        }
        else if (kv[0] == "width")
        {
            ArgsParams.width = std::stoi(kv[1]);
        }
        else if (kv[0] == "height")
        {
            ArgsParams.height = std::stoi(kv[1]);
        }
        else if (kv[0] == "encoder")
        {
            if (kv[1] == "libx264")
            {
                ArgsParams.encoder = VIDEO_ENCODER_X264;
            }
            else if (kv[1] == "h264_qsv")
            {
                ArgsParams.encoder = VIDEO_ENCODER_QSV;
            }
            else
            {
                ArgsParams.encoder = VIDEO_ENCODER_CUDA;
            }
        }
        else if (kv[0] == "decoder")
        {
            if (kv[1] == "h264")
            {
                ArgsParams.decoder = VIDEO_DECODER_H264;
            }
            else if (kv[1] == "d3d11va")
            {
                ArgsParams.decoder = VIDEO_DECODER_D3D11;
            }
            else if (kv[1] == "h264_qsv")
            {
                ArgsParams.decoder = VIDEO_DECODER_QSV;
            }
            else
            {
                ArgsParams.decoder = VIDEO_DECODER_CUDA;
            }
        }
        else if (kv[0] == "server")
        {
            ArgsParams.server = kv[1];
        }
    }
}

std::vector<std::string> Args::finds(std::string input, std::string delimiter)
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
