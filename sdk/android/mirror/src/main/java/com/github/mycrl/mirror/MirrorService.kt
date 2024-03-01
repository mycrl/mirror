package com.github.mycrl.mirror

import android.media.AudioRecord
import android.media.AudioTrack
import android.util.Log
import android.view.Surface
import java.lang.Exception

typealias MirrorServiceConfigure = MirrorOptions;

interface MirrorAdapterConfigure {
    val video: Video.VideoEncoder.VideoEncoderConfigure
    val audio: Audio.AudioEncoder.AudioEncoderConfigure
}

abstract class MirrorReceiver {

    /**
     *  You need to provide a surface to the receiver, which will decode and render the received
     *  video stream to this surface.
     */
    abstract val surface: Surface;

    /**
     * You need to provide an audio track to the receiver, which will decode the received audio
     * stream and play it using this audio track.
     */
    abstract val track: AudioTrack?;

    /**
     * You can choose to implement this function, and the underlying transport layer will give you a c
     * opy of the audio and video data, with the `kind` parameter indicating the type of packet.
     */
    open fun sink(buf: ByteArray, kind: Int) {}

    /**
     * Called when the receiver is closed, the likely reason is because the underlying transport
     * layer has been disconnected, perhaps because the sender has been closed or the network is
     * disconnected.
     */
    open fun released() {}

    /**
     * Called when the receiver is created, this will pass you a wrapper for the underlying adapter,
     * and you can actively release this receiver by calling the release method of the adapter.
     */
    open fun onStart(adapter: ReceiverAdapterWrapper) {}
}

abstract class MirrorServiceObserver {

    /**
     * This function is called when another sender is found on the LAN, and you can not accept this
     * sender by returning null.
     */
    abstract fun accept(id: Int, ip: String): MirrorReceiver?;
}

/**
 * Create a mirror service, note that observer can be null, when observer is null, it will not
 * automatically respond to any sender push.
 */
class MirrorService constructor(
    private val configure: MirrorServiceConfigure,
    private val observer: MirrorServiceObserver?
) {
    private val mirror: Mirror = Mirror(configure, if (observer != null) {
        object : ReceiverAdapterFactory() {
            override fun connect(
                id: Int,
                ip: String,
                description: ByteArray
            ): ReceiverAdapter? {
                val receiver = observer.accept(id, ip)
                return if (receiver != null) {
                    object : ReceiverAdapter() {
                        private var isReleased: Boolean = false
                        private val codecDescription = CodecDescriptionFactory.decode(description)
                        private val videoDecoder = Video.VideoDecoder(
                            receiver.surface,
                            object : Video.VideoDecoder.VideoDecoderConfigure {
                                override val height = codecDescription.video.height
                                override val width = codecDescription.video.width
                            })

                        private val audioDecoder = if (receiver.track != null) {
                            Audio.AudioDecoder(
                                receiver.track!!,
                                object : Audio.AudioDecoder.AudioDecoderConfigure {
                                    override val sampleRate = codecDescription.audio.sampleRate
                                    override val channels = codecDescription.audio.channels
                                    override val bitRate = codecDescription.audio.bitRate
                                })
                        } else {
                            null
                        }

                        init {
                            videoDecoder.start()
                            audioDecoder?.start()
                            receiver.onStart(ReceiverAdapterWrapper { -> close() })
                        }

                        override fun sink(kind: Int, buf: ByteArray): Boolean {
                            try {
                                if (isReleased) {
                                    return false
                                }

                                when (kind) {
                                    StreamKind.Video -> {
                                        if (videoDecoder.isRunning) {
                                            videoDecoder.sink(buf)
                                        }
                                    }

                                    StreamKind.Audio -> {
                                        if (audioDecoder != null && audioDecoder.isRunning) {
                                            audioDecoder.sink(buf)
                                        }
                                    }
                                }

                                receiver.sink(buf, kind)
                                return true
                            } catch (e: Exception) {
                                Log.e(
                                    "com.github.mycrl.mirror",
                                    "Mirror ReceiverAdapter sink exception",
                                    e
                                )

                                return false
                            }
                        }

                        override fun close() {
                            try {
                                if (!isReleased) {
                                    isReleased = true
                                    audioDecoder?.release()
                                    videoDecoder.release()
                                    receiver.released()
                                }
                            } catch (e: Exception) {
                                Log.e(
                                    "com.github.mycrl.mirror",
                                    "Mirror ReceiverAdapter close exception",
                                    e
                                )
                            }
                        }
                    }
                } else {
                    null
                }
            }
        }
    } else {
        null
    })

    /**
     * Release this mirror instance.
     */
    fun release() {
        mirror.release()
    }

    /**
     * Creates an instance of a sender with an unlimited `id` parameter, this id is passed to all
     * receivers and is mainly used to provide receivers with identification of this sender.
     */
    fun createSender(
        id: Int,
        configure: MirrorAdapterConfigure,
        record: AudioRecord?
    ): MirrorSender {
        return MirrorSender(
            mirror.createSender(
                id,
                CodecDescriptionFactory.encode(
                    CodecDescriptionFactory.CodecDescription(
                        CodecDescriptionFactory.VideoDescription(
                            configure.video.width,
                            configure.video.height,
                        ),
                        CodecDescriptionFactory.AudioDescription(
                            configure.audio.sampleRate,
                            configure.audio.channels,
                            configure.audio.bitRate,
                        )
                    )
                ),
            ),
            configure,
            record,
        )
    }

    /**
     * Creating a receiver and connecting to a specific sender results in a more proactive connection
     * than auto-discovery, and the handshake will take less time.
     *
     * `port` The port number from the created sender.
     */
    fun createReceiver(
        ip: String,
        port: Int,
        configure: MirrorAdapterConfigure,
        observer: MirrorReceiver
    ) {
        var adapter: ReceiverAdapterWrapper? = null
        adapter = mirror.createReceiver("$ip:$port", object : ReceiverAdapter() {
            private var isReleased: Boolean = false
            private val videoDecoder = Video.VideoDecoder(
                observer.surface,
                object : Video.VideoDecoder.VideoDecoderConfigure {
                    override val height = configure.video.height
                    override val width = configure.video.width
                })

            private val audioDecoder = if (observer.track != null) {
                Audio.AudioDecoder(
                    observer.track!!,
                    object : Audio.AudioDecoder.AudioDecoderConfigure {
                        override val sampleRate = configure.audio.sampleRate
                        override val channels = configure.audio.channels
                        override val bitRate = configure.audio.bitRate
                    })
            } else {
                null
            }

            init {
                videoDecoder.start()
                audioDecoder?.start()
                observer.onStart(ReceiverAdapterWrapper { -> close() })
            }

            override fun sink(kind: Int, buf: ByteArray): Boolean {
                if (isReleased) {
                    return false
                }

                when (kind) {
                    StreamKind.Video -> {
                        if (videoDecoder.isRunning) {
                            videoDecoder.sink(buf)
                        }
                    }

                    StreamKind.Audio -> {
                        if (audioDecoder != null && audioDecoder.isRunning) {
                            audioDecoder.sink(buf)
                        }
                    }
                }

                observer.sink(buf, kind)
                return true
            }

            override fun close() {
                try {
                    if (!isReleased) {
                        isReleased = true
                        adapter?.release()
                        audioDecoder?.release()
                        videoDecoder.release()
                        observer.released()
                    }
                } catch (e: Exception) {
                    Log.e(
                        "com.github.mycrl.mirror",
                        "Mirror ReceiverAdapter close exception",
                        e
                    )
                }
            }
        })
    }
}

class MirrorSender constructor(
    private val sender: SenderAdapterWrapper,
    private val configure: MirrorAdapterConfigure,
    private val record: AudioRecord?,
) {
    private val videoEncoder: Video.VideoEncoder =
        Video.VideoEncoder(configure.video, object : ByteArraySinker() {
            override fun sink(info: StreamBufferInfo, buf: ByteArray) {
                sender.send(info, buf)
            }
        })

    private val audioEncoder: Audio.AudioEncoder =
        Audio.AudioEncoder(record, configure.audio, object : ByteArraySinker() {
            override fun sink(info: StreamBufferInfo, buf: ByteArray) {
                sender.send(info, buf)
            }
        })

    init {
        videoEncoder.start()
        audioEncoder.start()
    }

    /**
     * Get the surface inside the sender, you need to render the texture to this surface to pass the
     * screen to other receivers.
     */
    fun getSurface(): Surface? {
        return videoEncoder.getSurface()
    }

    /**
     * Push a single frame of data into the video encoder, note that the frame type needs to be the
     * same as the encoder configuration and you need to be aware of the input frame rate.
     */
    fun pushVideoFrame(frame: ByteArray) {
        videoEncoder.sink(frame)
    }

    fun pushAudioChunk(chunk: ByteArray) {
        audioEncoder.sink(chunk)
    }

    /**
     * Get the port that sender is bound to.
     */
    fun getPort(): Int {
        return sender.port
    }

    /**
     * Close and release this sender.
     */
    fun release() {
        audioEncoder.release()
        videoEncoder.release()
        sender.release()
    }
}