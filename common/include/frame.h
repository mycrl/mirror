//
//  codec.h
//  codec
//
//  Created by Panda on 2024/2/14.
//

#ifndef FRAME_H
#define FRAME_H
#pragma once

#include <stdint.h>

 struct VideoFrameRect
{
    size_t width;
    size_t height;
};

struct VideoFrame
{
    struct VideoFrameRect rect;
    uint8_t* data[2];
    size_t linesize[2];
};

struct AudioFrame
{
    uint32_t frames;
    uint8_t* data[2];
};

#endif /* FRAME_H */
