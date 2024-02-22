package com.github.mycrl.mirror

import android.media.AudioRecord
import android.media.AudioTrack
import android.media.MediaCodec
import android.media.MediaCodecInfo
import android.media.MediaFormat
import android.view.Surface
import com.ensarsarajcic.kotlinx.serialization.msgpack.MsgPack
import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable
import kotlinx.serialization.decodeFromByteArray
import kotlinx.serialization.encodeToByteArray
import java.lang.Exception
import java.nio.ByteBuffer

abstract class ByteArraySinker {
    abstract fun sink(info: StreamBufferInfo, buf: ByteArray);
}

class Video {
    class VideoEncoder constructor(
        private val configure: VideoEncoderConfigure,
        private val sinker: ByteArraySinker
    ) {
        public var isRunning: Boolean = false

        private val bufferInfo = MediaCodec.BufferInfo()
        private var surface: Surface
        private var codec: MediaCodec
        private var worker: Thread

        init {
            val format = MediaFormat.createVideoFormat(MediaFormat.MIMETYPE_VIDEO_AVC, configure.width, configure.height)
            format.setFloat(MediaFormat.KEY_I_FRAME_INTERVAL, 0.5F)
            format.setInteger(MediaFormat.KEY_BIT_RATE, configure.bitRate)
            format.setInteger(MediaFormat.KEY_FRAME_RATE, configure.frameRate)
            format.setInteger(MediaFormat.KEY_COLOR_FORMAT, configure.format)
            format.setInteger(MediaFormat.KEY_BITRATE_MODE, MediaCodecInfo.EncoderCapabilities.BITRATE_MODE_VBR)
            format.setInteger(MediaFormat.KEY_LEVEL, MediaCodecInfo.CodecProfileLevel.AVCProfileBaseline)
            format.setInteger(MediaFormat.KEY_PROFILE, MediaCodecInfo.CodecProfileLevel.AVCProfileBaseline)

            codec = MediaCodec.createEncoderByType(MediaFormat.MIMETYPE_VIDEO_AVC)
            codec.configure(format, null, null, MediaCodec.CONFIGURE_FLAG_ENCODE)
            surface = codec.createInputSurface()

            worker = Thread {
                val buffer = ByteArray(1024 * 1024)
                val streamBufferInfo = StreamBufferInfo(StreamKind.Video);

                while (isRunning) {
                    try {
                        val index = codec.dequeueOutputBuffer(bufferInfo, -1)
                        if (index >= 0) {
                            val outputBuffer = codec.getOutputBuffer(index)
                            if (outputBuffer != null && bufferInfo.size > 0) {
                                streamBufferInfo.flags = bufferInfo.flags;
                                outputBuffer.get(buffer, 0, bufferInfo.size)

                                sinker.sink(
                                    streamBufferInfo,
                                    buffer.sliceArray(bufferInfo.offset until bufferInfo.size),
                                )
                            }

                            codec.releaseOutputBuffer(index, false)
                        }
                    } catch (_: Exception) {
                        release()
                    }
                }
            }
        }

        fun sink(buf: ByteArray) {
            val index = codec.dequeueInputBuffer(-1)
            if (index >= 0) {
                codec.getInputBuffer(index)?.put(buf)
                codec.queueInputBuffer(index, 0, buf.size, 0, 0)
            }
        }

        fun getSurface(): Surface {
            return surface
        }

        fun start() {
            if (!isRunning) {
                isRunning = true

                codec.start()
                worker.start()
            }
        }

        fun release() {
            if (isRunning) {
                isRunning = false

                codec.flush()
                codec.stop()
                codec.release()
            }
        }

        interface VideoEncoderConfigure {

            /**
             * [MediaCodecInfo.CodecCapabilities](https://developer.android.com/reference/android/media/MediaCodecInfo.CodecCapabilities)
             */
            val format: Int;
            val width: Int;
            val height: Int;

            /**
             * [MediaFormat#KEY_BIT_RATE](https://developer.android.com/reference/android/media/MediaFormat#KEY_BIT_RATE)
             */
            val bitRate: Int;

            /**
             * [MediaFormat#KEY_FRAME_RATE](https://developer.android.com/reference/android/media/MediaFormat#KEY_FRAME_RATE)
             */
            val frameRate: Int;
        }
    }

    class VideoDeocder constructor(private val surface: Surface, private val configure: VideoDecoderConfigure) {
        public var isRunning: Boolean = false

        private val bufferInfo = MediaCodec.BufferInfo()
        private var codec: MediaCodec
        private var worker: Thread

        init {
            val format = MediaFormat.createVideoFormat(MediaFormat.MIMETYPE_VIDEO_AVC, configure.width, configure.height)
            format.setInteger(MediaFormat.KEY_COLOR_FORMAT, MediaCodecInfo.CodecCapabilities.COLOR_FormatSurface)
            format.setInteger(MediaFormat.KEY_BITRATE_MODE, MediaCodecInfo.EncoderCapabilities.BITRATE_MODE_VBR)
            format.setInteger(MediaFormat.KEY_LEVEL, MediaCodecInfo.CodecProfileLevel.AVCProfileBaseline)
            format.setInteger(MediaFormat.KEY_PROFILE, MediaCodecInfo.CodecProfileLevel.AVCProfileBaseline)

            codec = MediaCodec.createDecoderByType(MediaFormat.MIMETYPE_VIDEO_AVC)
            codec.configure(format, surface, null, 0)
            surface.lockHardwareCanvas()

            worker = Thread {
                while (isRunning) {
                    try {
                        val index = codec.dequeueOutputBuffer(bufferInfo, -1)
                        if (index >= 0) {
                            codec.releaseOutputBuffer(index, true)
                        }
                    } catch (_: Exception) {
                        release()
                    }
                }
            }
        }

        fun sink(buf: ByteArray) {
            try {
                val index = codec.dequeueInputBuffer(-1)
                if (index >= 0) {
                    codec.getInputBuffer(index)?.put(buf)
                    codec.queueInputBuffer(index, 0, buf.size, 0, 0)
                }
            } catch (_: Exception) {
                release()
            }
        }

        fun start() {
            if (!isRunning) {
                isRunning = true

                codec.start()
                worker.start()
            }
        }

        fun release() {
            if (isRunning) {
                isRunning = false

                codec.flush()
                codec.stop()
                codec.release()
            }
        }

        interface VideoDecoderConfigure {
            val width: Int;
            val height: Int;
        }
    }
}

class Audio {
    class AudioDecoder constructor(private val track: AudioTrack, private val configure: AudioDecoderConfigure) {
        public var isRunning: Boolean = false

        private val bufferInfo = MediaCodec.BufferInfo()
        private var codec: MediaCodec
        private var worker: Thread

        init {
            val format = MediaFormat.createAudioFormat(MediaFormat.MIMETYPE_AUDIO_AMR_WB, configure.sampleRate, configure.channels)
            format.setInteger(MediaFormat.KEY_BIT_RATE, configure.bitRate)

            codec = MediaCodec.createDecoderByType(MediaFormat.MIMETYPE_AUDIO_AMR_WB)
            codec.configure(format, null, null, 0)

            worker = Thread {
                val buf = ByteArray(1024 * 10)

                while (isRunning) {
                    try {
                        val index = codec.dequeueOutputBuffer(bufferInfo, -1)
                        if (index >= 0) {
                            val outputBuffer = codec.getOutputBuffer(index)
                            if (outputBuffer != null && bufferInfo.size > 0) {
                                outputBuffer.get(buf, 0, bufferInfo.size)
                                track.write(buf, 0, bufferInfo.size)
                            }

                            codec.releaseOutputBuffer(index, false)
                        }
                    } catch (_: Exception) {
                        release()
                    }
                }
            }
        }

        fun sink(buf: ByteArray) {
            val index = codec.dequeueInputBuffer(-1)
            if (index >= 0) {
                codec.getInputBuffer(index)?.put(buf)
                codec.queueInputBuffer(index, 0, buf.size, 0, 0)
            }
        }

        fun start() {
            if (!isRunning) {
                isRunning = true

                codec.start()
                worker.start()
                track.play()
            }
        }

        fun release() {
            if (isRunning) {
                isRunning = false

                track.stop()
                track.release()
                codec.flush()
                codec.stop()
                codec.release()
            }
        }

        interface AudioDecoderConfigure {
            val sampleRate: Int;
            val channels: Int;
            val bitRate: Int;
        }
    }

    class AudioEncoder constructor(
        private val record: AudioRecord,
        private val configure: AudioEncoderConfigure,
        private val sinker: ByteArraySinker
    ) {
        public var isRunning: Boolean = false

        private val bufferInfo = MediaCodec.BufferInfo()
        private var codec: MediaCodec
        private var worker: Thread
        private var recorder: Thread

        private val minBufferSize = AudioRecord.getMinBufferSize(
            configure.sampleRate,
            configure.channalConfig,
            configure.sampleBits
        )

        init {
            val format = MediaFormat.createAudioFormat(MediaFormat.MIMETYPE_AUDIO_AMR_WB, configure.sampleRate, configure.channels)
            format.setInteger(MediaFormat.KEY_BIT_RATE, configure.bitRate)

            codec = MediaCodec.createEncoderByType(MediaFormat.MIMETYPE_AUDIO_AMR_WB)
            codec.configure(format, null, null, MediaCodec.CONFIGURE_FLAG_ENCODE)

            worker = Thread {
                val buffer = ByteArray(1024 * 1024)
                val streamBufferInfo = StreamBufferInfo(StreamKind.Audio);

                while (isRunning) {
                    try {
                        val index = codec.dequeueOutputBuffer(bufferInfo, -1)
                        if (index >= 0) {
                            val outputBuffer = codec.getOutputBuffer(index)
                            if (outputBuffer != null && bufferInfo.size > 0) {
                                streamBufferInfo.flags = bufferInfo.flags;
                                outputBuffer.get(buffer, 0, bufferInfo.size)

                                sinker.sink(
                                    streamBufferInfo,
                                    buffer.sliceArray(bufferInfo.offset until bufferInfo.size),
                                )
                            }

                            codec.releaseOutputBuffer(index, false)
                        }
                    } catch (_: Exception) {
                        release()
                    }
                }
            }

            recorder = Thread {
                while (isRunning) {
                    try {
                        val buf = ByteBuffer.allocateDirect(minBufferSize)
                        val size = record.read(buf, buf.capacity(), AudioRecord.READ_BLOCKING)
                        if (size > 0) {
                            val index = codec.dequeueInputBuffer(-1)
                            if (index >= 0) {
                                codec.getInputBuffer(index)?.put(buf)
                                codec.queueInputBuffer(
                                    index,
                                    0,
                                    size,
                                    System.nanoTime() / 1000,
                                    0
                                )
                            }
                        }
                    } catch (_: Exception) {
                        release()
                    }
                }
            }
        }

        fun start() {
            if (!isRunning) {
                isRunning = true

                codec.start()
                worker.start()
                recorder.start()
                record.startRecording()
            }
        }

        fun release() {
            if (isRunning) {
                isRunning = false

                record.stop()
                codec.flush()
                codec.stop()
                codec.release()
            }
        }

        interface AudioEncoderConfigure {

            /**
             * [AudioFormat#ENCODING_PCM_16BIT](https://developer.android.com/reference/android/media/AudioFormat#ENCODING_PCM_16BIT)
             */
            val sampleBits: Int;

            /**
             * [AudioFormat#SAMPLE_RATE_UNSPECIFIED](https://developer.android.com/reference/android/media/AudioFormat#SAMPLE_RATE_UNSPECIFIED)
             */
            val sampleRate: Int;

            /**
             * [AudioFormat#CHANNEL_IN_MONO](https://developer.android.com/reference/android/media/AudioFormat#CHANNEL_IN_MONO)
             */
            val channalConfig: Int;

            /**
             * Number of audio channels, such as mono or stereo (dual channel)
             */
            val channels: Int;

            /**
             * [MediaFormat#KEY_BIT_RATE](https://developer.android.com/reference/android/media/MediaFormat#KEY_BIT_RATE)
             */
            val bitRate: Int;
        }
    }
}

class CodecDescriptionFactory {
    @Serializable
    data class CodecDescription(
        @SerialName("v") val video: VideoDescription,
        @SerialName("a") val audio: AudioDescription,
    )

    @Serializable
    data class VideoDescription(
        @SerialName("w") val width: Int,
        @SerialName("h") val height: Int,
    )

    @Serializable
    data class AudioDescription(
        @SerialName("sr") val sampleRate: Int,
        @SerialName("cs") val channels: Int,
        @SerialName("br") val bitRate: Int,
    )

    companion object {
        fun encode(value: CodecDescription): ByteArray {
            return MsgPack.encodeToByteArray(value)
        }

        fun decode(value: ByteArray): CodecDescription {
            return MsgPack.decodeFromByteArray<CodecDescription>(value)
        }
    }
}