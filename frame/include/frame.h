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

typedef struct
{
    size_t width;
    size_t height;
} FrameRect;

typedef struct
{
    FrameRect rect;
    uint8_t* data[2];
    size_t linesize[2];
} VideoFrame;

#endif /* FRAME_H */
