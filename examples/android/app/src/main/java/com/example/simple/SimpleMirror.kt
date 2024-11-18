package com.example.simple

// noinspection SuspiciousImport
import android.R
import android.annotation.SuppressLint
import android.app.Activity
import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.app.Service
import android.content.Intent
import android.graphics.BitmapFactory
import android.hardware.display.DisplayManager
import android.hardware.display.VirtualDisplay
import android.media.AudioAttributes
import android.media.AudioFormat
import android.media.AudioPlaybackCaptureConfiguration
import android.media.AudioRecord
import android.media.AudioTrack
import android.media.MediaCodecInfo
import android.media.projection.MediaProjection
import android.media.projection.MediaProjectionManager
import android.os.Binder
import android.os.IBinder
import android.util.DisplayMetrics
import android.util.Log
import android.view.Surface
import com.github.mycrl.hylarana.Audio
import com.github.mycrl.hylarana.Discovery
import com.github.mycrl.hylarana.DiscoveryService
import com.github.mycrl.hylarana.DiscoveryServiceQueryObserver
import com.github.mycrl.hylarana.HylaranaOptions
import com.github.mycrl.hylarana.HylaranaReceiver
import com.github.mycrl.hylarana.HylaranaReceiverObserver
import com.github.mycrl.hylarana.HylaranaSender
import com.github.mycrl.hylarana.HylaranaSenderConfigure
import com.github.mycrl.hylarana.HylaranaSenderObserver
import com.github.mycrl.hylarana.HylaranaService
import com.github.mycrl.hylarana.HylaranaStrategy
import com.github.mycrl.hylarana.HylaranaStrategyType
import com.github.mycrl.hylarana.Properties
import com.github.mycrl.hylarana.Video

class Notify(service: SimpleHylaranaService) {
    companion object {
        private const val NotifyId = 100
        private const val NotifyChannelId = "SimpleHylarana"
        private const val NotifyChannelName = "SimpleHylarana"
    }

    init {
        val manager = service.getSystemService(Service.NOTIFICATION_SERVICE) as NotificationManager
        manager.createNotificationChannel(
            NotificationChannel(
                NotifyChannelId,
                NotifyChannelName,
                NotificationManager.IMPORTANCE_LOW
            )
        )

        val intent = Intent(service, MainActivity::class.java)
        val icon = BitmapFactory.decodeResource(service.resources, R.mipmap.sym_def_app_icon)
        val content =
            PendingIntent.getActivity(
                service,
                0,
                intent,
                PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT
            )

        val builder = Notification.Builder(service.applicationContext, NotifyChannelId)
        builder.setContentIntent(content)
        builder.setLargeIcon(icon)
        builder.setContentTitle("Screen recording")
        builder.setSmallIcon(R.mipmap.sym_def_app_icon)
        builder.setContentText("Recording screen......")
        builder.setWhen(System.currentTimeMillis())
        service.startForeground(NotifyId, builder.build())
    }
}

abstract class SimpleHylaranaServiceObserver() {
    abstract fun onConnected()

    abstract fun onReceiverClosed()
}

class SimpleHylaranaServiceBinder(private val service: SimpleHylaranaService) : Binder() {
    fun createSender(intent: Intent, displayMetrics: DisplayMetrics) {
        service.createSender(intent, displayMetrics)
    }

    fun createReceiver() {
        service.createReceiver()
    }

    fun setRenderSurface(surface: Surface) {
        Log.i("simple", "set render surface to service.")

        service.setOutputSurface(surface)
    }

    fun connect(strategy: HylaranaStrategy) {
        service.connect(strategy)
    }

    fun stopSender() {
        Log.i("simple", "stop sender.")

        service.stopSender()
    }

    fun stopReceiver() {
        Log.i("simple", "stop receiver.")

        service.stopReceiver()
    }

    fun setObserver(observer: SimpleHylaranaServiceObserver) {
        service.setObserver(observer)
    }
}

class SimpleHylaranaService : Service() {
    private var observer: SimpleHylaranaServiceObserver? = null
    private var mediaProjection: MediaProjection? = null
    private var virtualDisplay: VirtualDisplay? = null
    private var outputSurface: Surface? = null
    private var receiver: HylaranaReceiver? = null
    private var sender: HylaranaSender? = null
    private var discovery: DiscoveryService? = null
    private var strategy: HylaranaStrategy? = null

    override fun onBind(intent: Intent?): IBinder {
        return SimpleHylaranaServiceBinder(this)
    }

    override fun onDestroy() {
        super.onDestroy()
        sender?.release()
        mediaProjection?.stop()
        virtualDisplay?.release()

        Log.w("simple", "service destroy.")
    }

    fun connect(strategy: HylaranaStrategy) {
        this.strategy = strategy

        try {
            observer?.onConnected()
        } catch (e: Exception) {
            Log.e("simple", "Hylarana connect exception", e)
        }
    }

    fun stopSender() {
        discovery?.release()
        discovery = null

        sender?.release()
        sender = null
    }

    fun stopReceiver() {
        discovery?.release()
        discovery = null

        receiver?.release()
        receiver = null
    }

    fun setObserver(observer: SimpleHylaranaServiceObserver) {
        this.observer = observer
    }

    fun setOutputSurface(surface: Surface) {
        outputSurface = surface
    }

    fun createReceiver() {
        Log.i("simple", "create receiver.")

        discovery =
            Discovery()
                .query(
                    object : DiscoveryServiceQueryObserver() {
                        override fun resolve(addrs: Array<String>, properties: Properties) {
                            if (receiver == null) {
                                val sdp = Sdp.fromProperties(properties)
                                if (sdp.strategy.type == HylaranaStrategyType.DIRECT) {
                                    sdp.strategy.addr =
                                        addrs[0] + ":" + sdp.strategy.addr.split(":")[1]
                                }

                                receiver = HylaranaService.createReceiver(
                                    sdp.id,
                                    HylaranaOptions(strategy = sdp.strategy, mtu = 1500),
                                    object : HylaranaReceiverObserver() {
                                        override val surface = outputSurface!!
                                        override val track =
                                            AudioTrack.Builder()
                                                .setAudioAttributes(
                                                    AudioAttributes.Builder()
                                                        .setUsage(AudioAttributes.USAGE_MEDIA)
                                                        .setContentType(
                                                            AudioAttributes.CONTENT_TYPE_MUSIC
                                                        )
                                                        .build()
                                                )
                                                .setAudioFormat(
                                                    AudioFormat.Builder()
                                                        .setEncoding(AudioFormat.ENCODING_PCM_16BIT)
                                                        .setSampleRate(48000)
                                                        .setChannelMask(
                                                            AudioFormat.CHANNEL_OUT_MONO
                                                        )
                                                        .build()
                                                )
                                                .setPerformanceMode(
                                                    AudioTrack.PERFORMANCE_MODE_LOW_LATENCY
                                                )
                                                .setTransferMode(AudioTrack.MODE_STREAM)
                                                .setBufferSizeInBytes(48000 / 10 * 2)
                                                .build()

                                        override fun close() {
                                            super.close()
                                            stopReceiver()
                                            observer?.onReceiverClosed()

                                            Log.w("simple", "receiver is released.")
                                        }
                                    }
                                )
                            }
                        }
                    }
                )
    }

    @SuppressLint("MissingPermission")
    fun createSender(intent: Intent, displayMetrics: DisplayMetrics) {
        Notify(this)

        Log.i("simple", "create sender.")

        mediaProjection =
            (getSystemService(MEDIA_PROJECTION_SERVICE) as MediaProjectionManager)
                .getMediaProjection(Activity.RESULT_OK, intent)

        mediaProjection?.registerCallback(object : MediaProjection.Callback() {}, null)
        sender =
            strategy?.let {
                HylaranaService.createSender(
                    object : HylaranaSenderConfigure {
                        override val options = HylaranaOptions(strategy = it, mtu = 1500)

                        override val video =
                            object : Video.VideoEncoder.VideoEncoderConfigure {
                                override val format =
                                    MediaCodecInfo.CodecCapabilities.COLOR_FormatSurface
                                override var height = displayMetrics.heightPixels
                                override var width = displayMetrics.widthPixels
                                override val bitRate = 500 * 1024 * 8
                                override val frameRate = 60
                            }

                        override val audio =
                            object : Audio.AudioEncoder.AudioEncoderConfigure {
                                override val channalConfig = AudioFormat.CHANNEL_IN_MONO
                                override val sampleBits = AudioFormat.ENCODING_PCM_16BIT
                                override val sampleRate = 48000
                                override val bitRate = 64000
                                override val channels = 1
                            }
                    },
                    object : HylaranaSenderObserver() {
                        override val record =
                            AudioRecord.Builder()
                                .setAudioFormat(
                                    AudioFormat.Builder()
                                        .setSampleRate(48000)
                                        .setChannelMask(AudioFormat.CHANNEL_IN_MONO)
                                        .setEncoding(AudioFormat.ENCODING_PCM_16BIT)
                                        .build()
                                )
                                .setAudioPlaybackCaptureConfig(
                                    AudioPlaybackCaptureConfiguration.Builder(mediaProjection!!)
                                        .addMatchingUsage(AudioAttributes.USAGE_MEDIA)
                                        .addMatchingUsage(AudioAttributes.USAGE_GAME)
                                        .build()
                                )
                                .setBufferSizeInBytes(48000 / 10 * 2)
                                .build()

                        override fun close() {
                            super.close()

                            sender?.release()
                        }
                    }
                )
            }

        discovery =
            strategy?.let {
                Discovery()
                    .register(
                        3456,
                        Sdp(id = sender!!.getStreamId(), strategy = it).toProperties()
                    )
            }

        virtualDisplay =
            mediaProjection?.createVirtualDisplay(
                "HylaranaVirtualDisplayService",
                displayMetrics.widthPixels,
                displayMetrics.heightPixels,
                1,
                DisplayManager.VIRTUAL_DISPLAY_FLAG_AUTO_MIRROR,
                null,
                null,
                null
            )

        virtualDisplay?.surface = sender?.getSurface()
    }
}

data class Sdp(val id: String, val strategy: HylaranaStrategy) {
    fun toProperties(): Properties {
        return mapOf(
            "id" to id,
            "strategy" to strategy.type.toString(),
            "address" to strategy.addr,
        )
    }

    companion object {
        fun fromProperties(properties: Properties): Sdp {
            return Sdp(
                id = properties["id"] ?: throw Exception("not found id property"),
                strategy =
                HylaranaStrategy(
                    type =
                    (properties["strategy"]
                        ?: throw Exception("not found strategy property"))
                        .toInt(),
                    addr =
                    properties["address"] ?: throw Exception("not found address property")
                )
            )
        }
    }
}
