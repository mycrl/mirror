package com.github.mycrl.mirror

class StreamKind {
    companion object {
        const val Video = 0;
        const val Audio = 1;
    }
}

abstract class ReceiverAdapter {
    abstract fun sink(kind: Int, flags: Int, timestamp: Long, buf: ByteArray): Boolean
    abstract fun close()
}

data class StreamBufferInfo(val kind: Int) {
    var flags: Int = 0;
    var timestamp: Long = 0;
}

class SenderAdapterWrapper constructor(
    private val sendProc: (StreamBufferInfo, ByteArray) -> Unit,
    private val getMulticastProc: () -> Boolean,
    private val setMulticastProc: (Boolean) -> Unit,
    private val releaseProc: () -> Unit,
) {
    fun send(info: StreamBufferInfo, buf: ByteArray) {
        sendProc(info, buf)
    }

    fun getMulticast() : Boolean {
        return getMulticastProc()
    }

    fun setMulticast(isMulticast: Boolean) {
        setMulticastProc(isMulticast)
    }

    fun release() {
        releaseProc()
    }
}

class ReceiverAdapterWrapper constructor(private val releaser: () -> Unit) {
    /**
     * Close and release this receiver.
     */
    fun release() {
        releaser()
    }
}

class Mirror constructor(
    private val server: String,
    private val multicast: String,
    private val mtu: Int,
) {
    private var mirror: Long = 0

    init {
        mirror = createMirror(server, multicast, mtu)
        if (mirror == 0L) {
            throw Exception("failed to create mirror!")
        }
    }

    fun createSender(id: Int): SenderAdapterWrapper {
        val sender = createStreamSenderAdapter()
        if (sender == 0L) {
            throw Exception("failed to create sender adapter!")
        }

        createSender(mirror, id, sender)
        return SenderAdapterWrapper(
            { info, buf -> sendBufToSender(sender, info, buf) },
            { -> senderGetMulticast(sender) },
            { enable -> senderSetMulticast(sender, enable) },
            { -> releaseStreamSenderAdapter(sender) },
        )
    }

    fun createReceiver(id: Int, adapter: ReceiverAdapter): ReceiverAdapterWrapper {
        val receiver = createStreamReceiverAdapter(adapter)
        if (receiver == 0L) {
            throw Exception("failed to create receiver adapter!")
        }

        if (!createReceiver(mirror, id, receiver)) {
            throw Exception("failed to create mirror receiver adapter!")
        }

        return ReceiverAdapterWrapper { ->
            run {
                releaseStreamReceiverAdapter(receiver)
                adapter.close()
            }
        }
    }

    fun release() {
        if (mirror != 0L) {
            releaseMirror(mirror)
        }
    }

    companion object {
        init {
            System.loadLibrary("mirror_android")
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
      * Creates a mirror instance, the return value is a pointer, and you
        need to
      * check that the pointer is valid.
      */
    private external fun createMirror(
        server: String,
        multicast: String,
        mtu: Int,
    ): Long

     /**
      * Free the mirror instance pointer.
      */
    private external fun releaseMirror(mirror: Long)

     /**
      * Creates an instance of the stream sender adapter, the return value is
        a
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
        mirror: Long,
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
      *  was successful or not.
      */
    private external fun createReceiver(
        mirror: Long,
        id: Int,
        adapter: Long
    ): Boolean
}
