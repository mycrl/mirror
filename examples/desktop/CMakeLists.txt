cmake_minimum_required(VERSION 3.16)
project(example)

set(CMAKE_CXX_STANDARD 20)
set(CMAKE_CXX_STANDARD_REQUIRED True)

# Default build target is Debug.
if (NOT CMAKE_BUILD_TYPE)
    set(CMAKE_BUILD_TYPE "Debug")
endif()

set(THIRD_PARTY_DIR "${CMAKE_CURRENT_SOURCE_DIR}/third_party")

if(UNIX)
    if (NOT EXISTS "${THIRD_PARTY_DIR}")
        execute_process(COMMAND mkdir "${THIRD_PARTY_DIR}")
    endif()

    if (NOT EXISTS "${THIRD_PARTY_DIR}/sdl2")
        execute_process(
            WORKING_DIRECTORY "${THIRD_PARTY_DIR}"
            COMMAND bash -c "git clone --branch SDL2 https://github.com/libsdl-org/SDL sdl2")
    endif()
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

include_directories("../../sdk/desktop/include"
                    "../../sdk/renderer/include"
                    "../../frame/include")

if(UNIX)
    include_directories("${THIRD_PARTY_DIR}/sdl2/include")
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
                          mirror.dll.lib
                          renderer.dll.lib)
endif()

if(UNIX)
    target_link_libraries(example
                          libmirror.so
                          librenderer.so)
endif()
