cmake_minimum_required(VERSION 3.16)
project(example)

set(CMAKE_CXX_STANDARD 20)
set(CMAKE_CXX_STANDARD_REQUIRED True)

# Default build target is Debug.
if (NOT CMAKE_BUILD_TYPE)
    set(CMAKE_BUILD_TYPE "Debug")
endif()

if(MSVC)
    add_executable(example WIN32 src/main.cpp 
                    src/args.h 
                    src/args.cpp 
                    src/render.h 
                    src/render.cpp 
                    src/service.h 
                    src/service.cpp)
else()
    add_executable(example src/main.cpp 
                    src/args.h 
                    src/args.cpp 
                    src/render.h 
                    src/render.cpp 
                    src/service.h 
                    src/service.cpp)
endif()

include_directories("../../ffi/include")

if(UNIX)
    include_directories("/usr/include/SDL2"
                        "/usr/include")
endif()

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

if(UNIX)
    include_directories(example PRIVATE
                        "/usr/lib/x86_64-linux-gnu")
endif()

# Debug/Release target in build header.
if(CMAKE_BUILD_TYPE STREQUAL "Debug")
    add_compile_definitions(DEBUG)
endif()

# Build header.
if(WIN32)
    add_compile_definitions(WIN32 _WINDOWS)
else()
    add_compile_definitions(LINUX SDL_VIDEO_DRIVER_X11)
endif()

# Introduce other runtime necessary library files.
if(WIN32)
    target_link_libraries(example
                          ws2_32
                          userenv
                          ntdll
                          bcrypt
                          mirror.dll.lib)
endif()

if(UNIX)
    target_link_libraries(example
                          libmirror.so
                          SDL2)
endif()
