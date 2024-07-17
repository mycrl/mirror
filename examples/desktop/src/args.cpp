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