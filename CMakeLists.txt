cmake_minimum_required(VERSION 3.16)
project(mirror)

add_subdirectory(examples/desktop)

if(UNIX AND NOT APPLE)
    add_subdirectory(capture)
endif()
