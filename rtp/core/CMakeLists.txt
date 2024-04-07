cmake_minimum_required(VERSION 3.16)
project(rtp)

set(CMAKE_CXX_STANDARD 20)
set(CMAKE_CXX_STANDARD_REQUIRED True)

if (NOT CMAKE_BUILD_TYPE)
    set(CMAKE_BUILD_TYPE "Debug")
endif()

if(APPLE)
    execute_process(COMMAND bash -c "brew --prefix jrtplib"
                    OUTPUT_VARIABLE JRTP_PREFIX
                    OUTPUT_STRIP_TRAILING_WHITESPACE)
    set(JRTP_PREFIX ${JRTP_PREFIX})
endif()

set(SOURCES
    src/rtp.h
    src/rtp.cpp)

add_library(rtp STATIC ${SOURCES})

include_directories("/Users/mycrl/Downloads/jrtplib-3.11.2/src"
                    "/Users/mycrl/Downloads/jthread-1.3.3")

target_link_directories(rtp PRIVATE
                        "${JRTP_PREFIX}/lib")
target_link_libraries(rtp 
                      jrtp)

if(MSVC)
    if(CMAKE_BUILD_TYPE STREQUAL "Release")
        set_property(TARGET rtp PROPERTY MSVC_RUNTIME_LIBRARY MultiThreaded)
    else()
        set_property(TARGET rtp PROPERTY MSVC_RUNTIME_LIBRARY MultiThreadedDebug)
    endif()
endif()

add_compile_definitions(RTP_BIG_ENDIAN
                        RTP_SUPPORT_IPV4MULTICAST
                        RTP_SUPPORT_THREAD
                        RTP_SUPPORT_SENDAPP
                        RTP_SUPPORT_MEMORYMANAGEMEN)

if(CMAKE_BUILD_TYPE STREQUAL "Debug")
    add_compile_definitions(DEBUG)
endif()
