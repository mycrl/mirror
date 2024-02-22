cmake_minimum_required(VERSION 3.16)
project(devices)

# Version 30 of obs requires at least c++ 17 support.
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

# Clone the obs-studio repository.
if (NOT EXISTS "${THIRD_PARTY_DIR}/obs-studio")
    if (WIN32)
        execute_process(
            WORKING_DIRECTORY "${THIRD_PARTY_DIR}"
            COMMAND powershell.exe -Command "git clone \
                                             --branch release/30.0 \
                                             https://github.com/obsproject/obs-studio")
    else()
        execute_process(
            WORKING_DIRECTORY "${THIRD_PARTY_DIR}"
            COMMAND bash -c "git clone \
                             --branch release/30.0 \
                             https://github.com/obsproject/obs-studio")
    endif()
endif()

add_executable(devices
    devices.c)

# Add third-party libraries to the search path.
include_directories("${THIRD_PARTY_DIR}/obs-studio/libobs")

# Add third-party library files.
target_link_directories(devices PRIVATE
                        "${THIRD_PARTY_DIR}")
target_link_libraries(devices
                      c++
                      "-framework obs")

# If it is WIN32, you need to set the runtime library.
if(MSVC)
    if(CMAKE_BUILD_TYPE STREQUAL "Release")
        set_property(TARGET devices PROPERTY MSVC_RUNTIME_LIBRARY MultiThreaded)
    else()
        set_property(TARGET devices PROPERTY MSVC_RUNTIME_LIBRARY MultiThreadedDebug)
    endif()
endif()

# Debug/Release target in build header.
if(CMAKE_BUILD_TYPE STREQUAL "Release")
    add_compile_definitions(NDEBUG)
else()
    add_compile_definitions(DEBUG)
endif()
