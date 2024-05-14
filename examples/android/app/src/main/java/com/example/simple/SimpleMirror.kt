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
import com.github.mycrl.mirror.Audio
import com.github.mycrl.mirror.MirrorAdapterConfigure
import com.github.mycrl.mirror.MirrorReceiver
import com.github.mycrl.mirror.MirrorSender
import com.github.mycrl.mirror.MirrorService
import com.github.mycrl.mirror.MirrorServiceObserver
import com.github.mycrl.mirror.ReceiverAdapterWrapper
import com.github.mycrl.mirror.Video

class Notify(service: SimpleMirrorService) {
    companion object {
        private const val NotifyId = 100
        private const val NotifyChannelId = "SimpleMirror"
        private const val NotifyChannelName = "SimpleMirror"
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
        val content = PendingIntent.getActivity(
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

class SimpleMirrorServiceBinder(private val service: SimpleMirrorService) : Binder() {
    fun createSender(intent: Intent, displayMetrics: DisplayMetrics) {
        service.createSender(intent, displayMetrics)
    }

    fun createReceiver(addr: String) {
        service.createReceiver(addr)
    }

    fun setRenderSurface(surface: Surface) {
        Log.i("simple", "set render surface to service.")

        service.setOutputSurface(surface)
    }

    fun registerReceivedHandler(handle: (Int, String) -> Unit) {
        service.registerReceivedHandler(handle)
    }

    fun registerReceivedReleaseHandler(handle: (Int, String) -> Unit) {
        service.registerReceivedReleaseHandler(handle)
    }

    fun stopSender() {
        Log.i("simple", "stop sender.")

        service.stopSender()
    }

    fun stopReceiver() {
        Log.i("simple", "stop receiver.")

        service.stopReceiver()
    }
}

class SimpleMirrorService : Service() {
    private var receivedReleaseHandler: ((Int, String) -> Unit)? = null
    private var receivedHandler: ((Int, String) -> Unit)? = null
    private var mediaProjection: MediaProjection? = null
    private var virtualDisplay: VirtualDisplay? = null
    private var outputSurface: Surface? = null
    private var sender: MirrorSender? = null

    companion object {
        private val VideoConfigure = object : Video.VideoEncoder.VideoEncoderConfigure {
            override val format = MediaCodecInfo.CodecCapabilities.COLOR_FormatSurface
            override val bitRate = 500 * 1024 * 8
            override val frameRate = 60
            override var height = 720
            override var width = 1280
        }

        private val AudioConfigure = object : Audio.AudioEncoder.AudioEncoderConfigure {
            override val channalConfig = AudioFormat.CHANNEL_IN_MONO
            override val sampleBits = AudioFormat.ENCODING_PCM_16BIT
            override val sampleRate = 48000
            override val bitRate = 64000
            override val channels = 1
        }
    }

    private var receiverAdapter: ReceiverAdapterWrapper? = null
    private val mirror: MirrorService =
        MirrorService(1400, "239.0.0.1", "0.0.0.0:3200", object : MirrorServiceObserver() {
            override fun accept(id: Int, ip: String): MirrorReceiver {
                receivedHandler?.let { it(id, ip) }

                return object : MirrorReceiver() {
                    override val track = createAudioTrack()
                    override val surface = outputSurface!!

                    override fun released() {
                        super.released()
                        receivedReleaseHandler?.let { it(id, ip) }

                        Log.w("simple", "receiver is released.")
                    }

                    override fun onStart(adapter: ReceiverAdapterWrapper) {
                        super.onStart(adapter)

                        receiverAdapter = adapter
                    }
                }
            }
        })

    override fun onBind(intent: Intent?): IBinder {
        return SimpleMirrorServiceBinder(this)
    }

    override fun onDestroy() {
        super.onDestroy()
        mirror.release()
        sender?.release()
        mediaProjection?.stop()
        virtualDisplay?.release()

        Log.w("simple", "service destroy.")
    }

    fun stopSender() {
        sender?.release()
    }

    fun stopReceiver() {
        receiverAdapter?.release()
    }

    fun registerReceivedReleaseHandler(handle: (Int, String) -> Unit) {
        receivedReleaseHandler = handle
    }

    fun registerReceivedHandler(handle: (Int, String) -> Unit) {
        receivedHandler = handle
    }

    fun setOutputSurface(surface: Surface) {
        outputSurface = surface
    }

    fun createReceiver(addr: String) {
        Log.i("simple", "create receiver.")

        val (_, port) = addr.split(":")
        mirror.createReceiver("0.0.0.0:${port.toInt()}", object : MirrorAdapterConfigure {
            override val video = VideoConfigure
            override val audio = AudioConfigure
        }, object : MirrorReceiver() {
            override val track = createAudioTrack()
            override val surface = outputSurface!!

            override fun released() {
                super.released()
                receivedReleaseHandler?.let { it(0, addr) }

                Log.w("simple", "receiver is released.")
            }

            override fun onStart(adapter: ReceiverAdapterWrapper) {
                super.onStart(adapter)

                receiverAdapter = adapter
            }
        })
    }

    fun createSender(intent: Intent, displayMetrics: DisplayMetrics) {
        Notify(this)

        Log.i("simple", "create sender.")

        VideoConfigure.width = displayMetrics.widthPixels
        VideoConfigure.height = displayMetrics.heightPixels
        mediaProjection =
            (getSystemService(MEDIA_PROJECTION_SERVICE) as MediaProjectionManager).getMediaProjection(
                Activity.RESULT_OK,
                intent
            )

        mediaProjection?.registerCallback(object : MediaProjection.Callback() {}, null)
        sender = mirror.createSender(
            0,
            "0.0.0.0:8080",
            object : MirrorAdapterConfigure {
                override val video = VideoConfigure
                override val audio = AudioConfigure
            },
            createAudioRecord()
        )

        virtualDisplay = mediaProjection?.createVirtualDisplay(
            "MirrorVirtualDisplayService",
            VideoConfigure.width, VideoConfigure.height, 1,
            DisplayManager.VIRTUAL_DISPLAY_FLAG_AUTO_MIRROR,
            null, null, null
        )

        virtualDisplay?.surface = sender?.getSurface()
    }

    private fun createAudioTrack(): AudioTrack {
        val attr = AudioAttributes.Builder()
        attr.setUsage(AudioAttributes.USAGE_MEDIA)
        attr.setContentType(AudioAttributes.CONTENT_TYPE_MUSIC)

        val format = AudioFormat.Builder()
        format.setEncoding(AudioFormat.ENCODING_PCM_16BIT)
        format.setSampleRate(AudioConfigure.sampleRate)
        format.setChannelMask(AudioFormat.CHANNEL_OUT_MONO)

        val builder = AudioTrack.Builder()
        builder.setAudioAttributes(attr.build())
        builder.setAudioFormat(format.build())
        builder.setPerformanceMode(AudioTrack.PERFORMANCE_MODE_LOW_LATENCY)
        builder.setTransferMode(AudioTrack.MODE_STREAM)
        builder.setBufferSizeInBytes(
            AudioTrack.getMinBufferSize(
                AudioConfigure.sampleRate,
                AudioFormat.CHANNEL_OUT_MONO,
                AudioFormat.ENCODING_PCM_16BIT
            ) * 2
        )

        return builder.build()
    }

    @SuppressLint("MissingPermission")
    private fun createAudioRecord(): AudioRecord {
        val format = AudioFormat.Builder()
        format.setSampleRate(AudioConfigure.sampleRate)
        format.setChannelMask(AudioFormat.CHANNEL_IN_MONO)
        format.setEncoding(AudioFormat.ENCODING_PCM_16BIT)

        val configure = AudioPlaybackCaptureConfiguration.Builder(mediaProjection!!)
        configure.addMatchingUsage(AudioAttributes.USAGE_MEDIA)
        configure.addMatchingUsage(AudioAttributes.USAGE_GAME)

        val builder = AudioRecord.Builder()
        builder.setAudioFormat(format.build())
        builder.setAudioPlaybackCaptureConfig(configure.build())
        builder.setBufferSizeInBytes(
            AudioRecord.getMinBufferSize(
                AudioConfigure.sampleRate,
                AudioFormat.CHANNEL_IN_MONO,
                AudioFormat.ENCODING_PCM_16BIT
            ) * 2
        )

        return builder.build()
    }
}
