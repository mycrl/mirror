//
//  rtp.h
//  rtp
//
//  Created by Panda on 2024/4/3.
//

#ifndef rtp_h
#define rtp_h
#pragma once

#ifdef WIN32
#define EXPORT __declspec(dllexport)
#else
#define EXPORT
#endif

#include <rtpsession.h>
#include <rtppacket.h>
#include <rtpsessionparams.h>
#include <rtpudpv4transmitter.h>
#include <rtpipv4address.h>
#include <rtperrors.h>

typedef struct
{
    uint8_t* buf;
    size_t size;
} Packet;

typedef struct
{
    jrtplib::RTPSession session;
    Packet* packet;
} RTP;

extern "C"
{

EXPORT RTP* create_sender(uint32_t bind_ip,
                          uint16_t bind_port,
                          uint32_t dest_ip,
                          uint16_t dest_port);
                          
EXPORT RTP* create_receiver(uint32_t bind_ip,
                            uint16_t bind_port,
                            uint32_t dest_ip,
                            uint16_t dest_port);

EXPORT void get_latest_error(char* msg);
EXPORT void close_rtp(RTP* rtp);
EXPORT bool send_packet(RTP* rtp, Packet* pkt);
EXPORT bool lock_poll_thread(RTP* rtp);
EXPORT bool unlock_poll_thread(RTP* rtp);
EXPORT bool goto_first_source(RTP* rtp);
EXPORT bool goto_next_source(RTP* rtp);
EXPORT jrtplib::RTPPacket* get_next_packet(RTP* rtp);
EXPORT Packet* get_packet_ref(RTP* rtp, jrtplib::RTPPacket* pkt);
EXPORT void unref_packet(RTP* rtp, jrtplib::RTPPacket* pkt);

}

#endif /* rtp_h */
