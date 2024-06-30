cmake_minimum_required(VERSION 3.16)
project(capture)

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

if (NOT EXISTS "${THIRD_PARTY_DIR}/obs.lib")
    if (WIN32)
        execute_process(
            WORKING_DIRECTORY "${THIRD_PARTY_DIR}"
            COMMAND powershell.exe -Command "Invoke-WebRequest \
                -Uri https://github.com/mycrl/distributions/releases/download/distributions/obs-windows-x64.lib \
                -OutFile obs.lib")
    endif()
endif()

if (NOT EXISTS "${THIRD_PARTY_DIR}/obs-studio")
    if (WIN32)
        execute_process(
            WORKING_DIRECTORY "${THIRD_PARTY_DIR}"
            COMMAND powershell.exe -Command "git clone --branch release/30.1 https://github.com/obsproject/obs-studio")
    endif()
endif()

add_library(capture STATIC 
            capture.cpp
            capture.h
            camera.h 
            camera.cpp)

include_directories("${THIRD_PARTY_DIR}/obs-studio"
                    "../../common/include")

target_link_directories(capture PRIVATE "${THIRD_PARTY_DIR}")
target_link_libraries(capture obs)

# If it is WIN32, you need to set the runtime library.
if(MSVC)
    if(CMAKE_BUILD_TYPE STREQUAL "Release")
        set_property(TARGET capture PROPERTY MSVC_RUNTIME_LIBRARY MultiThreaded)
    else()
        set_property(TARGET capture PROPERTY MSVC_RUNTIME_LIBRARY MultiThreadedDebug)
    endif()
endif()

# Build header.
if(WIN32)
    add_compile_definitions(WIN32)
endif()
