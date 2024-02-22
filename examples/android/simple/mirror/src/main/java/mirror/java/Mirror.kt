package mirror.java

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
    abstract fun connect(id: Int, ip: String, description: ByteArray): ReceiverAdapter?
}

data class StreamBufferInfo(val kind: Int) {
    var flags: Int = 0;
}

/**
 * For the configuration of the mirror service, you need to specify a binding address
 * (ip address and port number) for the underlying layer to be able to bind to the specified address.
 */
data class MirrorOptions(val bind: String) {
    /**
     * Set up the packet filter. The string must match appropriate syntax for packet filter setup.
     */
    var fec: String = "fec,layout:even,rows:20,cols:10,arq:always";

    /**
     * Maximum send bandwidth
     */
    var maxBandwidth: Int = -1;

    /**
     * Connect timeout.
     */
    var timeout: Int = 5000;

    /**
     * The latency value in the receiving direction of the socket. min value = 20
     */
    var latency: Int = 20;

    /**
     * Flow Control limits the maximum number of packets "in flight" - payload (data) packets that
     * were sent but reception is not yet acknowledged with an ACK control packet.
     */
    var fc: Int = 25600;

    /**
     * Maximum Segment Size. Used for buffer allocation and rate calculation using packet counter
     * assuming fully filled packets. Each party can set its own MSS value independently. During a
     * handshake the parties exchange MSS values, and the lowest is used.
     *
     * Generally on the internet MSS is 1500 by default. This is the maximum size of a UDP packet
     * and can be only decreased, unless you have some unusual dedicated network settings. MSS is
     * not to be confused with the size of the UDP payload or SRT payload - this size is the size
     * of the IP packet, including the UDP and SRT headers.
     */
    var mtu: Int = 1500;
}

class SenderAdapterWrapper constructor(
    private val sender: (StreamBufferInfo, ByteArray) -> Unit,
    private val releaser: () -> Unit,
    public val port: Int,
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
    private val options: MirrorOptions,
    private val adapterFactory: ReceiverAdapterFactory?
) {
    private var mirror: Long = 0

    init {
        mirror = createMirror(
            options, if (adapterFactory != null) {
                createStreamReceiverAdapterFactory(adapterFactory)
            } else {
                0L
            }
        )

        if (mirror == 0L) {
            throw Exception("failed to create mirror!")
        }
    }

    fun createSender(id: Int, description: ByteArray): SenderAdapterWrapper {
        val sender = createStreamSenderAdapter()
        if (sender == 0L) {
            throw Exception("failed to create sender adapter!")
        }

        val port = createSender(mirror, id, description, sender)
        if (port == -1) {
            throw Exception("failed to create mirror sender adapter!")
        }

        return SenderAdapterWrapper(
            { info, buf -> sendBufToSender(sender, info, buf) },
            { -> releaseStreamSenderAdapter(sender) },
            port,
        )
    }

    fun createReceiver(port: Int, adapter: ReceiverAdapter): ReceiverAdapterWrapper {
        val receiver = createStreamReceiverAdapter(adapter)
        if (receiver == 0L) {
            throw Exception("failed to create receiver adapter!")
        }

        if (!createReceiver(mirror, port, receiver)) {
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
        options: MirrorOptions,
        adapterFactory: Long
    ): Long

    private external fun releaseMirror(mirror: Long)

    private external fun createStreamSenderAdapter(): Long

    private external fun releaseStreamSenderAdapter(adapter: Long)

    private external fun createSender(
        mirror: Long,
        id: Int,
        description: ByteArray,
        adapter: Long
    ): Int

    private external fun sendBufToSender(
        adapter: Long,
        info: StreamBufferInfo,
        buf: ByteArray,
    )

    private external fun createReceiver(
        mirror: Long,
        port: Int,
        adapter: Long
    ): Boolean
}
