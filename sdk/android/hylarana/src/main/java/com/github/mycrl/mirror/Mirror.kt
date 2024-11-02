package com.github.mycrl.hylarana

class StreamKind {
    companion object {
        const val VIDEO = 0
        const val AUDIO = 1
    }
}

abstract class ReceiverAdapter {
    abstract fun sink(kind: Int, flags: Int, timestamp: Long, buf: ByteArray): Boolean
    abstract fun online()
    abstract fun close()
}

data class StreamBufferInfo(val kind: Int) {
    var flags: Int = 0
    var timestamp: Long = 0
}

class SenderAdapterWrapper(
    private val sendProc: (StreamBufferInfo, ByteArray) -> Unit,
    private val getMulticastProc: () -> Boolean,
    private val setMulticastProc: (Boolean) -> Unit,
    private val releaseProc: () -> Unit,
) {
    fun send(info: StreamBufferInfo, buf: ByteArray) {
        sendProc(info, buf)
    }

    fun getMulticast(): Boolean {
        return getMulticastProc()
    }

    fun setMulticast(isMulticast: Boolean) {
        setMulticastProc(isMulticast)
    }

    fun release() {
        releaseProc()
    }
}

class ReceiverAdapterWrapper(private val releaser: () -> Unit) {
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
    private var hylarana: Long = 0L

    init {
        hylarana = createHylarana(server, multicast, mtu)
        if (hylarana == 0L) {
            throw Exception("failed to create hylarana!")
        }
    }

    fun createSender(id: Int): SenderAdapterWrapper {
        var sender = createStreamSenderAdapter()
        if (sender == 0L || hylarana == 0L) {
            throw Exception("failed to create sender adapter!")
        }

        createSender(hylarana, id, sender)
        return SenderAdapterWrapper(
            { info, buf ->
                run {
                    if (sender != 0L) {
                        sendBufToSender(sender, info, buf)
                    }
                }
            },
            {
                run {
                    if (sender != 0L) senderGetMulticast(sender) else false
                }
            },
            { enable ->
                run {
                    if (sender != 0L) {
                        senderSetMulticast(sender, enable)
                    }
                }
            },
            {
                run {
                    if (sender != 0L) {
                        releaseStreamSenderAdapter(sender)
                        sender = 0L
                    }
                }
            },
        )
    }

    fun createReceiver(id: Int, adapter: ReceiverAdapter): ReceiverAdapterWrapper {
        var receiver = createStreamReceiverAdapter(adapter)
        if (receiver == 0L || hylarana == 0L) {
            throw Exception("failed to create receiver adapter!")
        }

        if (!createReceiver(hylarana, id, receiver)) {
            throw Exception("failed to create hylarana receiver adapter!")
        }

        return ReceiverAdapterWrapper {
            run {
                if (receiver != 0L) {
                    releaseStreamReceiverAdapter(receiver)
                    adapter.close()
                    receiver = 0L
                }
            }
        }
    }

    fun release() {
        if (hylarana != 0L) {
            releaseHylarana(hylarana)
            hylarana = 0L
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
    private external fun createStreamReceiverAdapter(adapter: ReceiverAdapter): Long

    /**
     * Free the stream receiver adapter instance pointer.
     */
    private external fun releaseStreamReceiverAdapter(adapter: Long)

    /**
     * Creates a hylarana instance, the return value is a pointer, and you need to
     * check that the pointer is valid.
     */
    private external fun createHylarana(
        server: String,
        multicast: String,
        mtu: Int,
    ): Long

    /**
     * Free the hylarana instance pointer.
     */
    private external fun releaseHylarana(hylarana: Long)

    /**
     * Creates an instance of the stream sender adapter, the return value is a
     * pointer and you need to check if the pointer is valid.
     */
    private external fun createStreamSenderAdapter(): Long

    /**
     * Get whether the sender uses multicast transmission
     */
    private external fun senderGetMulticast(adapter: Long): Boolean

    /**
     * Set whether the sender uses multicast transmission
     */
    private external fun senderSetMulticast(adapter: Long, isMulticast: Boolean)

    /**
     * Release the stream sender adapter.
     */
    private external fun releaseStreamSenderAdapter(adapter: Long)

    /**
     * Creates the sender, the return value indicates whether the creation
     * was successful or not.
     */
    private external fun createSender(
        hylarana: Long,
        id: Int,
        adapter: Long
    )

    /**
     * Sends the packet to the sender instance.
     */
    private external fun sendBufToSender(
        adapter: Long,
        info: StreamBufferInfo,
        buf: ByteArray,
    )

    /**
     * Creates the receiver, the return value indicates whether the creation
     * was successful or not.
     */
    private external fun createReceiver(
        hylarana: Long,
        id: Int,
        adapter: Long
    ): Boolean
}
