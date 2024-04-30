cmake_minimum_required(VERSION 3.16)
project(example)

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

if (NOT EXISTS "${THIRD_PARTY_DIR}/sdl")
    if (WIN32)
        execute_process(
            WORKING_DIRECTORY "${THIRD_PARTY_DIR}"
            COMMAND powershell.exe -Command "git clone --branch SDL2 https://github.com/libsdl-org/SDL.git sdl")
    else()
    endif()
endif()

if (NOT EXISTS "${THIRD_PARTY_DIR}/SDL2.lib")
    if (WIN32)
        execute_process(
            WORKING_DIRECTORY "${THIRD_PARTY_DIR}"
            COMMAND powershell.exe -Command "Invoke-WebRequest \
                                            -Uri https://github.com/mycrl/distributions/releases/download/distributions/SDL2.lib \
                                            -OutFile ./SDL2.lib")
    else()
    endif()
endif()

if(MSVC)
    add_executable(example WIN32 src/main.cpp)
else()
    add_executable(example src/main.cpp)
endif()

target_link_directories(example PRIVATE "${THIRD_PARTY_DIR}")
include_directories("../../sdk/desktop/include"
                    "${THIRD_PARTY_DIR}/sdl/include"
                    "../../common/include")

# If it is WIN32, you need to set the runtime library.
if(MSVC)
    if(CMAKE_BUILD_TYPE STREQUAL "Release")
        set_property(TARGET example PROPERTY MSVC_RUNTIME_LIBRARY MultiThreaded)
    else()
        set_property(TARGET example PROPERTY MSVC_RUNTIME_LIBRARY MultiThreadedDebug)
    endif()
endif()

if(CMAKE_BUILD_TYPE STREQUAL "Debug")
    target_link_directories(example PRIVATE "../../target/debug")
else()
    target_link_directories(example PRIVATE "../../target/release")
endif()

# Debug/Release target in build header.
if(CMAKE_BUILD_TYPE STREQUAL "Debug")
    add_compile_definitions(DEBUG)
endif()

# Build header.
if(WIN32)
    add_compile_definitions(WIN32
                            _WINDOWS)
endif()

# Introduce other runtime necessary library files.
if(WIN32)
    target_link_libraries(example
                          ws2_32
                          userenv
                          ntdll
                          bcrypt
                          SDL2
                          mirror.dll.lib)
endif()
