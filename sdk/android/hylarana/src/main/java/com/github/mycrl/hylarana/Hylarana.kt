package com.github.mycrl.hylarana

/**
 * Data Stream Receiver Adapter
 *
 * Used to receive data streams from the network.
 */
internal abstract class HylaranaReceiverAdapterObserver {
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

/**
 * STREAM_TYPE_VIDEO | STREAM_TYPE_AUDIO
 */
data class StreamBufferInfo(val type: Int) {
    var flags: Int = 0
    var timestamp: Long = 0
}

/**
 * transport strategy
 */
data class TransportStrategy(
    /**
     * STRATEGY_DIRECT | STRATEGY_RELAY | STRATEGY_MULTICAST
     */
    val type: Int,
    /**
     * socket address
     */
    var addr: String
)

data class TransportOptions(
    val strategy: TransportStrategy,
    /**
     * see: [Maximum_transmission_unit](https://en.wikipedia.org/wiki/Maximum_transmission_unit)
     */
    val mtu: Int
)

class HylaranaSenderAdapter(
    private val id: String,
    private val sendHandle: (StreamBufferInfo, ByteArray) -> Boolean,
    private val releaseHandle: () -> Unit,
) {
    /**
     * get sender stream id.
     */
    fun getId(): String {
        return id
    }

    /**
     * send stream buffer to sender.
     */
    fun send(info: StreamBufferInfo, buf: ByteArray): Boolean {
        return sendHandle(info, buf)
    }

    /**
     * Close and release this sender.
     */
    fun release() {
        releaseHandle()
    }
}

class HylaranaReceiverAdapter(private val releaseHandle: () -> Unit) {
    /**
     * Close and release this receiver.
     */
    fun release() {
        releaseHandle()
    }
}

internal class Hylarana {
    companion object {
        init {
            System.loadLibrary("hylarana")
        }
    }

    fun createSender(
        options: TransportOptions
    ): HylaranaSenderAdapter {
        var sender = createTransportSender(options)
        if (sender == 0L) {
            throw Exception("failed to create transport sender")
        }

        val id = getTransportSenderId(sender)
        return HylaranaSenderAdapter(
            id,
            { info, buf ->
                if (sender != 0L) {
                    if (!sendStreamBufferToTransportSender(sender, info, buf)) {
                        sender = 0L

                        false
                    } else {
                        true
                    }
                } else {
                    false
                }
            },
            {
                run {
                    if (sender != 0L) {
                        val ptr = sender
                        sender = 0L

                        releaseTransportSender(ptr)
                    }
                }
            },
        )
    }

    fun createReceiver(
        id: String, options: TransportOptions, observer: HylaranaReceiverAdapterObserver
    ): HylaranaReceiverAdapter {
        var receiver = createTransportReceiver(id, options, observer)
        if (receiver == 0L) {
            throw Exception("failed to create transport receiver")
        }

        return HylaranaReceiverAdapter {
            run {
                if (receiver != 0L) {
                    val ptr = receiver
                    receiver = 0L

                    releaseTransportReceiver(ptr)
                }
            }
        }
    }

    /**
     * Creates the sender, the return value indicates whether the creation
     * was successful or not.
     */
    private external fun createTransportSender(
        options: TransportOptions,
    ): Long

    /**
     * get transport sender id.
     */
    private external fun getTransportSenderId(
        sender: Long
    ): String

    /**
     * Sends the packet to the sender instance.
     */
    private external fun sendStreamBufferToTransportSender(
        sender: Long,
        info: StreamBufferInfo,
        buf: ByteArray,
    ): Boolean

    /**
     * release transport sender.
     */
    private external fun releaseTransportSender(sender: Long)

    /**
     * Creates the receiver, the return value indicates whether the creation
     * was successful or not.
     */
    private external fun createTransportReceiver(
        id: String,
        options: TransportOptions,
        observer: HylaranaReceiverAdapterObserver,
    ): Long

    /**
     * release transport receiver.
     */
    private external fun releaseTransportReceiver(sender: Long)
}
