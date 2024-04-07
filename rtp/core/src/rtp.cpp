//
//  rtp.cpp
//  rtp
//
//  Created by Panda on 2024/4/3.
//

#include "rtp.h"

#include <cstring>

int RESULT_CODE = 0;

void get_latest_error(char* msg)
{
    auto ret = jrtplib::RTPGetErrorString(RESULT_CODE);
    std::strcpy(msg, ret.c_str());
}

void release_rtp(RTP* rtp)
{
    if (rtp->packet != nullptr)
    {
        delete rtp->packet;
    }
    
    delete rtp;
}

void close_rtp(RTP* rtp)
{
    rtp->session.BYEDestroy(jrtplib::RTPTime(10, 0), nullptr, 0);
    rtp->session.AbortWait();
    release_rtp(rtp);
}

RTP* create_sender(uint32_t bind_ip,
                   uint16_t bind_port,
                   uint32_t dest_ip,
                   uint16_t dest_port)
{
    jrtplib::RTPSessionParams session_params;
    session_params.SetUsePollThread(true);
    session_params.SetNeedThreadSafety(true);
    session_params.SetAcceptOwnPackets(true);
    session_params.SetOwnTimestampUnit(1.0 / 10.0);
    
    jrtplib::RTPUDPv4TransmissionParams transport_params;
    transport_params.SetPortbase(bind_port);
    transport_params.SetBindIP(bind_ip);
    transport_params.SetMulticastTTL(255);
    
    RTP* rtp = new RTP();
    RESULT_CODE = rtp->session.Create(session_params,
                                      &transport_params,
                                      jrtplib::RTPTransmitter::TransmissionProtocol::IPv4UDPProto);
    if (RESULT_CODE < 0)
    {
        release_rtp(rtp);
        return nullptr;
    }
    
    jrtplib::RTPIPv4Address addr(dest_ip, dest_port, true);
    RESULT_CODE = rtp->session.AddDestination(addr);
    if (RESULT_CODE < 0)
    {
        release_rtp(rtp);
        return nullptr;
    }
    
    return rtp;
}

RTP* create_receiver(uint32_t bind_ip,
                     uint16_t bind_port,
                     uint32_t dest_ip,
                     uint16_t dest_port)
{
    jrtplib::RTPSessionParams session_params;
    session_params.SetUsePollThread(true);
    session_params.SetNeedThreadSafety(true);
    session_params.SetAcceptOwnPackets(true);
    session_params.SetOwnTimestampUnit(1.0 / 10.0);
    
    jrtplib::RTPUDPv4TransmissionParams transport_params;
    transport_params.SetPortbase(bind_port);
    transport_params.SetBindIP(bind_ip);
    
    RTP* rtp = new RTP();
    rtp->packet = new Packet();
    
    RESULT_CODE = rtp->session.Create(session_params,
                                      &transport_params,
                                      jrtplib::RTPTransmitter::TransmissionProtocol::IPv4UDPProto);
    if (RESULT_CODE < 0)
    {
        release_rtp(rtp);
        return nullptr;
    }

    jrtplib::RTPIPv4Address addr(dest_ip, dest_port, true);
    RESULT_CODE = rtp->session.AddDestination(addr);
    if (RESULT_CODE < 0)
    {
        release_rtp(rtp);
        return nullptr;
    }
    
    return rtp;
}

bool send_packet(RTP* rtp, Packet* pkt)
{
    RESULT_CODE = rtp->session.SendPacket((void*)pkt->buf, pkt->size, 0, false, 10);
    return RESULT_CODE >= 0;
}

bool lock_poll_thread(RTP* rtp)
{
    RESULT_CODE = rtp->session.BeginDataAccess();
    return RESULT_CODE >= 0;
}

bool unlock_poll_thread(RTP* rtp)
{
    RESULT_CODE = rtp->session.EndDataAccess();
    return RESULT_CODE >= 0;
}

bool goto_first_source(RTP* rtp)
{
    return rtp->session.GotoFirstSourceWithData();
}

bool goto_next_source(RTP* rtp)
{
    return rtp->session.GotoNextSourceWithData();
}

jrtplib::RTPPacket* get_next_packet(RTP* rtp)
{
    return rtp->session.GetNextPacket();
}

Packet* get_packet_ref(RTP* rtp, jrtplib::RTPPacket* pkt)
{
    rtp->packet->buf = pkt->GetPayloadData();
    rtp->packet->size = pkt->GetPayloadLength();
    return rtp->packet;
}

void unref_packet(RTP* rtp, jrtplib::RTPPacket* pkt)
{
    rtp->session.DeletePacket(pkt);
}
