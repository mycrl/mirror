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

typedef struct
{
	int width;
	int height;
} Size;

typedef struct
{
	Size size;
#ifdef WIN32
	HWND hwnd;
	void* d3d_device;
    void* d3d_device_context;
#endif // WIN32
} RendererDescriptor;

typedef const void* WindowHandle;
typedef const void* Render;

#ifndef WIN32

/**
 * Initialize the environment, which must be initialized before using the SDK.
 */
EXPORT bool renderer_startup();

#endif // !WIN32

/**
 * Creating a window renderer.
 */
EXPORT Render renderer_create(RendererDescriptor options);

/**
 * Push the video frame into the renderer, which will update the window texture.
 */
EXPORT bool renderer_on_video(Render render, VideoFrame* frame);

/**
 * Push the audio frame into the renderer, which will append to audio queue.
 */
EXPORT bool renderer_on_audio(Render render, AudioFrame* frame);

/**
 * Adjust the size of the renderer. When the window size changes, the internal 
 * size of the renderer needs to be updated, otherwise this will cause 
 * abnormal rendering.
 */
EXPORT bool renderer_resise(Render render, Size size);

/**
 * Destroy the window renderer.
 */
EXPORT void renderer_destroy(Render render);

#endif // RENDERER
