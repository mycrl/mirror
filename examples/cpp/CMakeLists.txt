cmake_minimum_required(VERSION 3.16)
project(example)

set(CMAKE_CXX_STANDARD 20)
set(CMAKE_CXX_STANDARD_REQUIRED True)

# Default build target is Debug.
if (NOT CMAKE_BUILD_TYPE)
    set(CMAKE_BUILD_TYPE "Debug")
endif()

# Enabling Objective-C++ on Macos.
if(APPLE)
    enable_language(OBJCXX)
    set_source_files_properties(src/main.cpp PROPERTIES LANGUAGE OBJCXX)
endif()

# Windows requires a win32 subsystem.
if(MSVC)
    add_executable(example WIN32 src/main.cpp src/cli.h)
else()
    add_executable(example src/main.cpp src/cli.h)
endif()

# Add third-party libraries to the search path.
include_directories("../../ffi/include")

if(APPLE)
    include_directories("/opt/homebrew/opt/sdl2/include/SDL2")
elseif(UNIX AND NOT APPLE)
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

if(APPLE)
    target_link_directories(example PRIVATE "/opt/homebrew/opt/sdl2/lib")
elseif(UNIX AND NOT APPLE)
    target_link_directories(example PRIVATE "/usr/lib/x86_64-linux-gnu")
endif()

# Introduce other runtime necessary library files.
if(MSVC)
    target_link_libraries(example
                          ws2_32
                          userenv
                          ntdll
                          bcrypt
                          mirror.dll.lib)
elseif(UNIX AND NOT APPLE)
    target_link_libraries(example
                          libmirror.so
                          SDL2)
elseif(UNIX)
    target_link_libraries(example
                          libmirror.so
                          SDL2
                          "-framework Cocoa")
endif()

# Debug/Release target in build header.
if(CMAKE_BUILD_TYPE STREQUAL "Debug")
    add_compile_definitions(DEBUG)
endif()

# Build header.
if(MSVC)
    add_compile_definitions(WIN32 
                            _WINDOWS)
elseif(APPLE)
    add_compile_definitions(MACOS)
elseif(UNIX AND NOT APPLE)
    add_compile_definitions(LINUX 
                            SDL_VIDEO_DRIVER_X11)
endif()
