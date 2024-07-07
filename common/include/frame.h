//
//  codec.h
//  codec
//
//  Created by Panda on 2024/2/14.
//

#ifndef FRAME_H
#define FRAME_H
#pragma once

#ifdef __cplusplus
#include <cstddef>
#endif

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

enum AudioFormat
{
    AUDIO_NONE = -1,
    AUDIO_U8,          ///< unsigned 8 bits
    AUDIO_S16,         ///< signed 16 bits
    AUDIO_S32,         ///< signed 32 bits
    AUDIO_FLT,         ///< float
    AUDIO_DBL,         ///< double
    AUDIO_U8P,         ///< unsigned 8 bits, planar
    AUDIO_S16P,        ///< signed 16 bits, planar
    AUDIO_S32P,        ///< signed 32 bits, planar
    AUDIO_FLTP,        ///< float, planar
    AUDIO_DBLP,        ///< double, planar
    AUDIO_S64,         ///< signed 64 bits
    AUDIO_S64P,        ///< signed 64 bits, planar
    AUDIO_NB           ///< Number of sample formats. DO NOT USE if linking dynamically
};

struct AudioFrame
{
    enum AudioFormat format;
    uint32_t frames;
    uint8_t* data;
};

#endif /* FRAME_H */
