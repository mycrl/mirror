package com.github.mycrl.hylarana

import android.media.AudioRecord
import android.media.AudioTrack
import android.util.Log
import android.view.Surface
import kotlin.Exception

typealias HylaranaOptions = TransportDescriptor
typealias HylaranaStrategy = TransportStrategy

class StreamType {
    companion object {
        const val VIDEO: Int = 0
        const val AUDIO: Int = 1
    }
}

class HylaranaStrategyType {
    companion object {
        const val DIRECT = 0
        const val RELAY = 1
        const val MULTICAST = 2
    }
}

interface HylaranaSenderConfigure {
    val video: Video.VideoEncoder.VideoEncoderConfigure
    val audio: Audio.AudioEncoder.AudioEncoderConfigure
    val options: HylaranaOptions
}

abstract class HylaranaReceiverObserver {

    /**
     *  You need to provide a surface to the receiver, which will decode and render the received
     *  video stream to this surface.
     */
    abstract val surface: Surface

    /**
     * You need to provide an audio track to the receiver, which will decode the received audio
     * stream and play it using this audio track.
     */
    abstract val track: AudioTrack?

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
    open fun close() {}
}

abstract class HylaranaSenderObserver {

    /**
     * A recorder that can record system sounds or other audio sources.
     */
    abstract val record: AudioRecord?

    /**
     * Called when the receiver is closed, the likely reason is because the underlying transport
     * layer has been disconnected, perhaps because the sender has been closed or the network is
     * disconnected.
     */
    open fun close() {}
}

/**
 * Create a hylarana service, note that observer can be null, when observer is null, it will not
 * automatically respond to any sender push.
 */
class HylaranaService {
    companion object {
        private val hylarana = Hylarana()

        /**
         * Creates an instance of a sender with an unlimited `id` parameter, this id is passed to all
         * receivers and is mainly used to provide receivers with identification of this sender.
         */
        fun createSender(
            configure: HylaranaSenderConfigure,
            observer: HylaranaSenderObserver
        ): HylaranaSender {
            return HylaranaSender(
                observer,
                hylarana.createSender(configure.options),
                configure,
                observer.record,
            )
        }

        /**
         * Creating a receiver and connecting to a specific sender results in a more proactive connection
         * than auto-discovery, and the handshake will take less time.
         *
         * `port` The port number from the created sender.
         */
        fun createReceiver(
            id: String,
            options: HylaranaOptions,
            observer: HylaranaReceiverObserver
        ): HylaranaReceiver {
            return HylaranaReceiver(
                hylarana.createReceiver(
                    id,
                    options,
                    object : HylaranaReceiverAdapterObserver() {
                        private var isReleased: Boolean = false
                        private val videoDecoder = Video.VideoDecoder(observer.surface)
                        private val audioDecoder = if (observer.track != null) {
                            Audio.AudioDecoder(observer.track!!)
                        } else {
                            null
                        }

                        init {
                            videoDecoder.start()
                            audioDecoder?.start()
                        }

                        override fun sink(
                            kind: Int,
                            flags: Int,
                            timestamp: Long,
                            buf: ByteArray
                        ): Boolean {
                            try {
                                if (isReleased) {
                                    return false
                                }

                                when (kind) {
                                    StreamType.VIDEO -> {
                                        if (videoDecoder.isRunning) {
                                            videoDecoder.sink(buf, flags, timestamp)
                                        }
                                    }

                                    StreamType.AUDIO -> {
                                        if (audioDecoder != null && audioDecoder.isRunning) {
                                            audioDecoder.sink(buf, flags, timestamp)
                                        }
                                    }
                                }

                                observer.sink(buf, kind)
                                return true
                            } catch (e: Exception) {
                                Log.e(
                                    "com.github.mycrl.hylarana",
                                    "Hylarana ReceiverAdapter sink exception",
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
                                    observer.close()
                                }
                            } catch (e: Exception) {
                                Log.e(
                                    "com.github.mycrl.hylarana",
                                    "Hylarana ReceiverAdapter close exception",
                                    e
                                )
                            }
                        }
                    }
                )
            )
        }
    }
}

class HylaranaReceiver(
    private val receiver: HylaranaReceiverAdapter
) {
    /**
     * Close and release this receiver.
     */
    fun release() {
        receiver.release()
    }
}

class HylaranaSender(
    private val observer: HylaranaSenderObserver,
    private val sender: HylaranaSenderAdapter,
    configure: HylaranaSenderConfigure,
    record: AudioRecord?,
) {
    private val videoEncoder: Video.VideoEncoder =
        Video.VideoEncoder(configure.video, object : ByteArraySinker() {
            override fun sink(info: StreamBufferInfo, buf: ByteArray) {
                if (!sender.send(info, buf)) {
                    observer.close()
                }
            }
        })

    private val audioEncoder: Audio.AudioEncoder =
        Audio.AudioEncoder(record, configure.audio, object : ByteArraySinker() {
            override fun sink(info: StreamBufferInfo, buf: ByteArray) {
                if (!sender.send(info, buf)) {
                    observer.close()
                }
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

    fun pushAudioFrame(chunk: ByteArray) {
        audioEncoder.sink(chunk)
    }

    /**
     * get sender stream id.
     */
    fun getStreamId(): String {
        return sender.getId()
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
