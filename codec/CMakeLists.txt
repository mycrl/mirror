cmake_minimum_required(VERSION 3.16)
project(codec)

set(CMAKE_CXX_STANDARD 20)
set(CMAKE_CXX_STANDARD_REQUIRED True)

set(THIRD_PARTY_DIR "${CMAKE_CURRENT_SOURCE_DIR}/third_party")

if (NOT EXISTS "${THIRD_PARTY_DIR}")
    execute_process(COMMAND mkdir "${THIRD_PARTY_DIR}")
endif()

if (NOT EXISTS "${THIRD_PARTY_DIR}/ffmpeg")
    execute_process(
        WORKING_DIRECTORY "${THIRD_PARTY_DIR}"
        COMMAND git clone --branch release/7.0 https://github.com/FFmpeg/FFmpeg ffmpeg)
endif()

if (NOT EXISTS "${THIRD_PARTY_DIR}/libyuv")
    execute_process(
        WORKING_DIRECTORY "${THIRD_PARTY_DIR}"
        COMMAND git clone --branch stable https://chromium.googlesource.com/libyuv/libyuv)
endif()

if (NOT EXISTS "${THIRD_PARTY_DIR}/media-sdk")
    execute_process(
        WORKING_DIRECTORY "${THIRD_PARTY_DIR}"
        COMMAND git clone https://github.com/Intel-Media-SDK/MediaSDK media-sdk)
endif()

add_library(codec STATIC 
            lib/h264.cpp
            lib/opus.cpp
            lib/codec.cpp
            lib/codec.h)

include_directories("${THIRD_PARTY_DIR}/ffmpeg"
                    "${THIRD_PARTY_DIR}/libyuv/include"
                    "${THIRD_PARTY_DIR}/media-sdk/api/include"
                    "../frame/include")

# If it is WIN32, you need to set the runtime library.
if(MSVC)
    if(CMAKE_BUILD_TYPE STREQUAL "Release")
        set_property(TARGET codec PROPERTY MSVC_RUNTIME_LIBRARY MultiThreaded)
    else()
        set_property(TARGET codec PROPERTY MSVC_RUNTIME_LIBRARY MultiThreadedDebug)
    endif()
endif()

# Build header.
if(WIN32)
    add_compile_definitions(WIN32)
endif()
