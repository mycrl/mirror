cmake_minimum_required(VERSION 3.16)
project(capture)

set(CMAKE_CXX_STANDARD 20)
set(CMAKE_CXX_STANDARD_REQUIRED True)

add_executable(capture
               lib/capture.cpp)

include_directories("/usr/include/libdrm"
                    "/usr/include")

add_compile_definitions(DEBUG)

target_link_directories(capture PRIVATE "/usr/lib/x86_64-linux-gnu")

target_link_libraries(capture drm gbm vulkan)
