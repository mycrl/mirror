//
// mirror.h
// mirror
//
// Created by Panda on 2024/4/1.
//

#ifndef MIRROR_H
#define MIRROR_H
#pragma once

#ifndef EXPORT
#ifdef WIN32
#define EXPORT __declspec(dllexport)
#else
#define EXPORT
#endif
#endif

#include <windows.h>

#include <stdint.h>
#include <stddef.h>

typedef enum
{
    VIDEO_FORMAT_BGRA,
    VIDEO_FORMAT_RGBA,
    VIDEO_FORMAT_NV12,
    VIDEO_FORMAT_I420,
} VideoFormat;

typedef enum
{
    VIDEO_SUB_FORMAT_D3D11,
    VIDEO_SUB_FORMAT_SW,
} VideoSubFormat;

typedef struct
{
    VideoFormat format;
    VideoSubFormat sub_format;
    uint32_t width;
    uint32_t height;
    void* data[3];
    size_t linesize[3];
} VideoFrame;

typedef struct
{
    int sample_rate;
    uint32_t frames;
    int16_t* data;
} AudioFrame;

typedef enum 
{
    RENDER_BACKEND_DX11,
    RENDER_BACKEND_WGPU,
} VideoRenderBackend;

typedef const void* WindowHandle;
typedef const void* Render;

EXPORT WindowHandle create_window_handle_for_win32(HWND hwnd, uint32_t width, uint32_t height);

/**
 * Destroy the window handle.
 */
EXPORT void window_handle_destroy(WindowHandle hwnd);

/**
 * Creating a window renderer.
 */
EXPORT Render renderer_create(WindowHandle hwnd, VideoRenderBackend backend);

/**
 * Push the video frame into the renderer, which will update the window texture.
 */
EXPORT bool renderer_on_video(Render render, VideoFrame* frame);

/**
 * Push the audio frame into the renderer, which will append to audio queue.
 */
EXPORT bool renderer_on_audio(Render render, AudioFrame* frame);

/**
 * Destroy the window renderer.
 */
EXPORT void renderer_destroy(Render render);

#endif // MIRROR_H
