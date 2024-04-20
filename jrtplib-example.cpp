//
//  main.cpp
//  jrtplib-example
//
//  Created by Panda on 2024/4/20.
//

#include <iostream>
#include <thread>
#include <WinSock2.h>

#include <rtppacket.h>
#include <rtpsession.h>
#include <rtptransmitter.h>
#include <rtpipv4address.h>
#include <rtpsessionparams.h>
#include <rtpsessionparams.h>
#include <rtpudpv4transmitter.h>

using namespace jrtplib;

void checkeeror(int status, const char* model)
{
    if (status < 0)
    {
        std::cerr << model << ": " << RTPGetErrorString(status) << std::endl;
        exit(status);
    }
}

static int sender()
{
    int ret;
    RTPSession session;

    RTPSessionParams session_params;
    session_params.SetOwnTimestampUnit(1.0 / 10.0);

    RTPUDPv4TransmissionParams trans_params;
    trans_params.SetBindIP(0);
    trans_params.SetPortbase(6000);
    trans_params.SetMulticastTTL(255);

    ret = session.Create(session_params, &trans_params, RTPTransmitter::TransmissionProtocol::IPv4UDPProto);
    checkeeror(ret, "Create");

    session.SetDefaultPayloadType(0);
    session.SetDefaultMark(false);
    session.SetDefaultTimestampIncrement(10);

    if (!session.SupportsMulticasting())
    {
        std::cerr << "not support multicastion" << std::endl;
        exit(-1);
    }

    uint8_t multicast[] = { 239, 0, 0, 1 };
    RTPIPv4Address addr(multicast, 6002);
    ret = session.AddDestination(addr);
    checkeeror(ret, "AddDestination");

    for (;;)
    {
        ret = session.SendPacket((void*)"1234567890", 10);
        checkeeror(ret, "SendPacket");

        printf("send packet \n");
        RTPTime::Wait(RTPTime(1, 0));
    }
}

static int receiver()
{
    int ret;
    RTPSession session;

    RTPSessionParams session_params;
    session_params.SetOwnTimestampUnit(1.0 / 10.0);
    session_params.SetUsePollThread(true);
    session_params.SetNeedThreadSafety(true);
    session_params.SetAcceptOwnPackets(true);
    session_params.SetReceiveMode(RTPTransmitter::ReceiveMode::AcceptAll);

    RTPUDPv4TransmissionParams trans_params;
    trans_params.SetBindIP(0);
    trans_params.SetPortbase(6002);

    ret = session.Create(session_params, &trans_params, RTPTransmitter::TransmissionProtocol::IPv4UDPProto);
    checkeeror(ret, "Create");

    session.SetDefaultPayloadType(0);
    session.SetDefaultMark(false);
    session.SetDefaultTimestampIncrement(10);

    if (!session.SupportsMulticasting())
    {
        std::cerr << "not support multicastion" << std::endl;
        exit(-1);
    }

    uint8_t multicast[] = { 239, 0, 0, 1 };
    RTPIPv4Address addr(multicast, 6002);
    ret = session.JoinMulticastGroup(addr);
    checkeeror(ret, "JoinMulticastGroup");

    for (;;)
    {
        ret = session.BeginDataAccess();
        checkeeror(ret, "BeginDataAccess");

        ret = session.Poll();
        checkeeror(ret, "Poll");

        if (session.GotoFirstSource())
        {
            do
            {
                RTPPacket* packet;
                while ((packet = session.GetNextPacket()) != 0)
                {
                    std::cout << "Got packet with extended sequence number "
                        << packet->GetExtendedSequenceNumber()
                        << " from SSRC " << packet->GetSSRC()
                        << std::endl;
                    session.DeletePacket(packet);
                }
            } while (session.GotoNextSource());
        }

        ret = session.EndDataAccess();
        checkeeror(ret, "EndDataAccess");
    }
}

int main(int argc, char* argv[]) {
    WSADATA ws_data;
    WSAStartup(MAKEWORD(2, 2), &ws_data);

    bool is_client = false;
    if (argc >= 2 && std::strcmp(argv[1], "-c") == 0) 
    {
        is_client = true;
    }

    if (is_client)
    {
        receiver();
    }
    else
    {
        sender();
    }

    WSACleanup();
    return 0;
}
