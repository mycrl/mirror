cmake_minimum_required(VERSION 3.16)
project(codec)

set(CMAKE_CXX_STANDARD 17)
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

# FFmpeg.

# In addition to windows, other platforms use package management
# to install ffmpeg.
if(APPLE)
    execute_process(COMMAND bash -c "brew --prefix ffmpeg"
                    OUTPUT_VARIABLE FFMPEG_PREFIX
                    OUTPUT_STRIP_TRAILING_WHITESPACE)
    set(FFMPEG_PREFIX ${FFMPEG_PREFIX})
elseif(WIN32)
    if (NOT EXISTS "${THIRD_PARTY_DIR}/ffmpeg-5.1.2-full_build-shared")
        execute_process(
            WORKING_DIRECTORY "${THIRD_PARTY_DIR}"
            COMMAND powershell.exe -Command "Invoke-WebRequest \
                                                -Uri https://www.gyan.dev/ffmpeg/builds/packages/ffmpeg-5.1.2-full_build-shared.7z \
                                                -OutFile ffmpeg.7z; \
                                             7z x ffmpeg.7z -aoa; \
                                             del ffmpeg.7z")
    endif()
    
    set(FFMPEG_PREFIX "${THIRD_PARTY_DIR}/ffmpeg-5.1.2-full_build-shared")
else()
    set(FFMPEG_PREFIX $ENV{FFMPEG_PREFIX})
endif()

add_library(codec SHARED
            src/codec.h
            src/video_encoder.cpp)

# Add third-party libraries to the search path.
include_directories("${FFMPEG_PREFIX}/include")

# Add third-party library files.
target_link_directories(codec PRIVATE
                        "${FFMPEG_PREFIX}/lib")
target_link_libraries(codec 
                      avcodec
                      avutil)

# If it is WIN32, you need to set the runtime library.
if(MSVC)
    if(CMAKE_BUILD_TYPE STREQUAL "Release")
        set_property(TARGET codec PROPERTY MSVC_RUNTIME_LIBRARY MultiThreaded)
    else()
        set_property(TARGET codec PROPERTY MSVC_RUNTIME_LIBRARY MultiThreadedDebug)
    endif()
endif()

# Debug/Release target in build header.
if(CMAKE_BUILD_TYPE STREQUAL "Release")
    add_compile_definitions(NDEBUG)
else()
    add_compile_definitions(DEBUG)
endif()

# Introduce other runtime necessary library files.
if(WIN32)
    target_link_libraries(codec
                        winmm
                        secur32
                        msdmo
                        dmoguids
                        wmcodecdspuuid
                        iphlpapi)
elseif(APPLE)
    target_link_libraries(codec
                        c++
                        "-framework Foundation"
                        "-framework AudioToolbox"
                        "-framework AudioUnit"
                        "-framework CoreServices"
                        "-framework CoreFoundation"
                        "-framework CoreAudio"
                        "-framework CoreGraphics")
elseif(UNIX AND NOT APPLE)
    target_link_libraries(codec
                        stdc++
                        X11)
endif()
