//
// renderer.h
// renderer
//
// Created by Panda on 2024/7/13.
//

#ifndef RENDERER_H
#define RENDERER_H
#pragma once

#ifndef EXPORT
#ifdef WIN32
#define EXPORT __declspec(dllexport)
#else
#define EXPORT
#endif
#endif

#ifdef WIN32
#include <windows.h>
#endif

#include <frame.h>

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

#endif // RENDERER
