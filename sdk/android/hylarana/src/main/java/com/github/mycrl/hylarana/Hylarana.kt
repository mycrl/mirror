package com.github.mycrl.hylarana

class StreamKind {
    companion object {
        const val VIDEO = 0
        const val AUDIO = 1
    }
}

/**
 * Data Stream Receiver Adapter
 *
 * Used to receive data streams from the network.
 */
abstract class HylaranaReceiverAdapterObserver {
    /**
     * Triggered when data arrives in the network.
     *
     * Note: If the buffer is empty, the current network connection has been closed or suddenly interrupted.
     */
    abstract fun sink(kind: Int, flags: Int, timestamp: Long, buf: ByteArray): Boolean

    /**
     * stream is closed.
     */
    abstract fun close()
}

data class StreamBufferInfo(val kind: Int) {
    var flags: Int = 0
    var timestamp: Long = 0
}

data class TransportDescriptor(
    /**
     * The IP address and port of the server, in this case the service refers to the mirror service
     */
    val server: String,
    /**
     * The multicast address used for multicasting, which is an IP address.
     */
    val multicast: String,
    /**
     * see: [Maximum_transmission_unit](https://en.wikipedia.org/wiki/Maximum_transmission_unit)
     */
    val mtu: Int
)

/**
 *
 */
data class StreamId(
    val uid: String,
    val port: Int
)

class HylaranaSenderAdapter(
    private val id: StreamId,
    private val sendProc: (StreamBufferInfo, ByteArray) -> Unit,
    private val getMulticastProc: () -> Boolean,
    private val setMulticastProc: (Boolean) -> Unit,
    private val releaseProc: () -> Unit,
) {
    /**
     * get sender stream id.
     */
    fun getStreamId(): StreamId {
        return id
    }

    /**
     * send stream buffer to sender.
     */
    fun send(info: StreamBufferInfo, buf: ByteArray) {
        sendProc(info, buf)
    }

    /**
     * get sender multicast.
     */
    fun getMulticast(): Boolean {
        return getMulticastProc()
    }

    /**
     * set sender multicast.
     */
    fun setMulticast(isMulticast: Boolean) {
        setMulticastProc(isMulticast)
    }

    /**
     * Close and release this sender.
     */
    fun release() {
        releaseProc()
    }
}

class HylaranaReceiverAdapter(private val releaser: () -> Unit) {
    /**
     * Close and release this receiver.
     */
    fun release() {
        releaser()
    }
}

class Hylarana(
    server: String,
    multicast: String,
    mtu: Int,
) {
    private var options: TransportDescriptor =
        TransportDescriptor(server = server, multicast = multicast, mtu = mtu)

    fun createSender(): HylaranaSenderAdapter {
        var sender = createSenderAdapter()
        return HylaranaSenderAdapter(
            createTransportSender(options, sender)
                ?: throw Exception("Failed to create transport sender"),
            { info, buf ->
                run {
                    if (sender != 0L) {
                        if (!senderAdapterSendBytes(sender, info, buf)) {
                            sender = 0L
                        }
                    }
                }
            },
            {
                run {
                    if (sender != 0L) senderAdapterGetMulticast(sender) else false
                }
            },
            { enable ->
                run {
                    if (sender != 0L) {
                        senderAdapterSetMulticast(sender, enable)
                    }
                }
            },
            {
                run {
                    if (sender != 0L) {
                        releaseSenderAdapter(sender)
                        sender = 0L
                    }
                }
            },
        )
    }

    fun createReceiver(
        id: StreamId,
        adapter: HylaranaReceiverAdapterObserver
    ): HylaranaReceiverAdapter {
        var receiver = createReceiverAdapter(adapter)
        if (receiver == 0L) {
            throw Exception("Failed to create transport receiver adapter")
        }

        if (!createTransportReceiver(id, options, receiver)) {
            throw Exception("Failed to create transport receiver")
        }

        return HylaranaReceiverAdapter {
            run {
                if (receiver != 0L) {
                    releaseReceiverAdapter(receiver)
                    adapter.close()
                    receiver = 0L
                }
            }
        }
    }

    companion object {
        init {
            System.loadLibrary("hylarana")
        }
    }

    /**
     * Create a stream receiver adapter where the return value is a
     * pointer to the instance, and you need to check that the returned
     * pointer is not Null.
     */
    private external fun createReceiverAdapter(adapter: HylaranaReceiverAdapterObserver): Long

    /**
     * Free the stream receiver adapter instance pointer.
     */
    private external fun releaseReceiverAdapter(adapter: Long)

    /**
     * Creates an instance of the stream sender adapter, the return value is a
     * pointer and you need to check if the pointer is valid.
     */
    private external fun createSenderAdapter(): Long

    /**
     * Get whether the sender uses multicast transmission
     */
    private external fun senderAdapterGetMulticast(adapter: Long): Boolean

    /**
     * Set whether the sender uses multicast transmission
     */
    private external fun senderAdapterSetMulticast(adapter: Long, isMulticast: Boolean)

    /**
     * Sends the packet to the sender instance.
     */
    private external fun senderAdapterSendBytes(
        adapter: Long,
        info: StreamBufferInfo,
        buf: ByteArray,
    ): Boolean

    /**
     * Release the stream sender adapter.
     */
    private external fun releaseSenderAdapter(adapter: Long)

    /**
     * Creates the sender, the return value indicates whether the creation
     * was successful or not.
     */
    private external fun createTransportSender(
        options: TransportDescriptor,
        adapter: Long
    ): StreamId?

    /**
     * Creates the receiver, the return value indicates whether the creation
     * was successful or not.
     */
    private external fun createTransportReceiver(
        id: StreamId,
        options: TransportDescriptor,
        adapter: Long
    ): Boolean
}
