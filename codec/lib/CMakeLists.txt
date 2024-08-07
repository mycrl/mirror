cmake_minimum_required(VERSION 3.16)
project(codec)

# Version 110 of webrtc native requires at least c++ 20 support.
set(CMAKE_CXX_STANDARD 20)
set(CMAKE_CXX_STANDARD_REQUIRED True)

# Default build target is Debug.
if (NOT CMAKE_BUILD_TYPE)
    set(CMAKE_BUILD_TYPE "Debug")
endif()

# Used to download or initialize third-party dependent libraries.
set(THIRD_PARTY_DIR "${CMAKE_CURRENT_SOURCE_DIR}/third_party")

# Check whether the third-party dependent library directory is created.
if (NOT EXISTS "${THIRD_PARTY_DIR}")
    if (WIN32)
        execute_process(COMMAND powershell.exe -Command "mkdir ${THIRD_PARTY_DIR}")
    else()
        execute_process(COMMAND mkdir "${THIRD_PARTY_DIR}")
    endif()
endif()

if (WIN32)
    set(ENV{PATH} "${THIRD_PARTY_DIR};$ENV{PATH}")
endif()

set(FFMPEG_PREFIX "${THIRD_PARTY_DIR}/ffmpeg")

if (NOT EXISTS "${FFMPEG_PREFIX}")
    if (WIN32)
        execute_process(
            WORKING_DIRECTORY "${THIRD_PARTY_DIR}"
            COMMAND powershell.exe -Command "Invoke-WebRequest \
                    -Uri https://github.com/mycrl/mirror/releases/download/distributions/ffmpeg-windows-x64-debug.zip \
                    -OutFile ffmpeg.zip")

        execute_process(
            WORKING_DIRECTORY "${THIRD_PARTY_DIR}"
            COMMAND powershell.exe -Command "Expand-Archive -Path ffmpeg.zip -DestinationPath ./")
    endif()
endif()

if (NOT EXISTS "${THIRD_PARTY_DIR}/libyuv")
    if (WIN32)
        execute_process(
            WORKING_DIRECTORY "${THIRD_PARTY_DIR}"
            COMMAND powershell.exe -Command "git clone --branch stable https://chromium.googlesource.com/libyuv/libyuv")
    endif()
endif()

add_library(codec STATIC 
            video_decode.cpp
            video_encode.cpp
            audio_decode.cpp
            audio_encode.cpp
            codec.cpp
            codec.h)

include_directories("${FFMPEG_PREFIX}/include"
                    "${THIRD_PARTY_DIR}/libyuv/include"
                    "../../common/include")

target_link_directories(codec PRIVATE "${FFMPEG_PREFIX}/lib")
target_link_libraries(codec avcodec 
                            avutil)

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
