package com.github.mycrl.mirror

class StreamKind {
    companion object {
        const val Video = 0;
        const val Audio = 1;
    }
}

abstract class ReceiverAdapter {
    abstract fun sink(kind: Int, buf: ByteArray): Boolean
    abstract fun close()
}

abstract class ReceiverAdapterFactory {
    abstract fun connect(id: Int, addr: String, description: ByteArray): ReceiverAdapter?
}

data class StreamBufferInfo(val kind: Int) {
    var flags: Int = 0;
}

class SenderAdapterWrapper constructor(
    private val sender: (StreamBufferInfo, ByteArray) -> Unit,
    private val releaser: () -> Unit,
) {
    fun send(info: StreamBufferInfo, buf: ByteArray) {
        sender(info, buf)
    }

    fun release() {
        releaser()
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
    private val bind: String,
    private val adapterFactory: ReceiverAdapterFactory?
) {
    private var mirror: Long = 0

    init {
        mirror = createMirror(
            bind, if (adapterFactory != null) {
                createStreamReceiverAdapterFactory(adapterFactory)
            } else {
                0L
            }
        )

        if (mirror == 0L) {
            throw Exception("failed to create mirror!")
        }
    }

    fun createSender(id: Int, mtu: Int, bind: String, description: ByteArray): SenderAdapterWrapper {
        val sender = createStreamSenderAdapter()
        if (sender == 0L) {
            throw Exception("failed to create sender adapter!")
        }

        createSender(mirror, id, mtu, bind, description, sender)
        return SenderAdapterWrapper(
            { info, buf -> sendBufToSender(sender, info, buf) },
            { -> releaseStreamSenderAdapter(sender) },
        )
    }

    fun createReceiver(bind: String, adapter: ReceiverAdapter): ReceiverAdapterWrapper {
        val receiver = createStreamReceiverAdapter(adapter)
        if (receiver == 0L) {
            throw Exception("failed to create receiver adapter!")
        }

        if (!createReceiver(mirror, bind, receiver)) {
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
            System.loadLibrary("mirror_exports")
        }
    }

    private external fun createStreamReceiverAdapterFactory(adapterFactory: ReceiverAdapterFactory): Long

    private external fun createStreamReceiverAdapter(adapter: ReceiverAdapter): Long

    private external fun releaseStreamReceiverAdapter(adapter: Long)

    private external fun createMirror(
        bind: String,
        adapterFactory: Long
    ): Long

    private external fun releaseMirror(mirror: Long)

    private external fun createStreamSenderAdapter(): Long

    private external fun releaseStreamSenderAdapter(adapter: Long)

    private external fun createSender(
        mirror: Long,
        id: Int,
        mtu: Int,
        bind: String,
        description: ByteArray,
        adapter: Long
    )

    private external fun sendBufToSender(
        adapter: Long,
        info: StreamBufferInfo,
        buf: ByteArray,
    )

    private external fun createReceiver(
        mirror: Long,
        bind: String,
        adapter: Long
    ): Boolean
}
