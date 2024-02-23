package com.example.simple

//noinspection SuspiciousImport
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
import android.view.Surface
import com.github.mycrl.mirror.Audio
import com.github.mycrl.mirror.MirrorAdapterConfigure
import com.github.mycrl.mirror.MirrorReceiver
import com.github.mycrl.mirror.MirrorSender
import com.github.mycrl.mirror.MirrorService
import com.github.mycrl.mirror.MirrorServiceConfigure
import com.github.mycrl.mirror.MirrorServiceObserver
import com.github.mycrl.mirror.ReceiverAdapterWrapper
import com.github.mycrl.mirror.Video

class ScreenCaptureServiceBinder(private val service: ScreenCaptureService) : Binder() {
    fun startup(intent: Intent) {
        service.start(intent)
    }

    fun setRenderSurface(surface: Surface) {
        service.renderSurface = surface
    }
}

class ScreenCaptureService : Service() {
    private val binder: ScreenCaptureServiceBinder = ScreenCaptureServiceBinder(this)
    private lateinit var mediaProjectionManager: MediaProjectionManager
    private lateinit var mediaProjection: MediaProjection
    private lateinit var virtualDisplay: VirtualDisplay
    private var sender: MirrorSender? = null
    var renderSurface: Surface? = null

    private val mirror: MirrorService = MirrorService(MirrorServiceConfigure("0.0.0.0:3200"), object : MirrorServiceObserver() {
        override fun accept(id: Int, ip: String): MirrorReceiver {
            return object : MirrorReceiver() {
                override val surface = renderSurface!!
                override val track = AudioTrack.Builder()
                    .setAudioAttributes(
                        AudioAttributes.Builder().setUsage(AudioAttributes.USAGE_MEDIA)
                            .setContentType(AudioAttributes.CONTENT_TYPE_MUSIC).build()
                    ).setAudioFormat(
                        AudioFormat.Builder().setEncoding(AudioFormat.ENCODING_PCM_16BIT)
                            .setSampleRate(16000).setChannelMask(AudioFormat.CHANNEL_OUT_MONO)
                            .build()
                    ).setBufferSizeInBytes(
                        AudioTrack.getMinBufferSize(
                            16000,
                            AudioFormat.CHANNEL_OUT_MONO,
                            AudioFormat.ENCODING_PCM_16BIT
                        )
                    )
                    .setPerformanceMode(AudioTrack.PERFORMANCE_MODE_LOW_LATENCY)
                    .setTransferMode(AudioTrack.MODE_STREAM)
                    .build()

                override fun onStart(adapter: ReceiverAdapterWrapper) {

                }

                override fun released() {

                }
            }
        }
    })

    @SuppressLint("MissingPermission")
    fun start(intent: Intent) {
        startNotification()

        mediaProjectionManager =
            getSystemService(MEDIA_PROJECTION_SERVICE) as MediaProjectionManager
        mediaProjection = mediaProjectionManager.getMediaProjection(Activity.RESULT_OK, intent)
        mediaProjection.registerCallback(object : MediaProjection.Callback() {
            override fun onStop() {
                super.onStop()
            }
        }, null)

        virtualDisplay = mediaProjection.createVirtualDisplay(
            "MirrorVirtualDisplayService",
            2560, 1600, 1,
            DisplayManager.VIRTUAL_DISPLAY_FLAG_AUTO_MIRROR,
            null, null, null
        )

        sender = mirror.createSender(
            0, object : MirrorAdapterConfigure {
                override val video = object : Video.VideoEncoder.VideoEncoderConfigure {
                    override val format = MediaCodecInfo.CodecCapabilities.COLOR_FormatSurface
                    override val width = 2560
                    override val height = 1600
                    override val frameRate = 60
                    override val bitRate = 7000 * 1024
                }

                override val audio = object : Audio.AudioEncoder.AudioEncoderConfigure {
                    override val channels = 1
                    override val bitRate = 1000 * 10
                    override val sampleRate = 16000
                    override val channalConfig = AudioFormat.CHANNEL_IN_MONO
                    override val sampleBits = AudioFormat.ENCODING_PCM_16BIT
                }
            }, AudioRecord.Builder()
                .setAudioFormat(
                    AudioFormat.Builder().setSampleRate(16000)
                        .setChannelMask(AudioFormat.CHANNEL_IN_MONO)
                        .setEncoding(AudioFormat.ENCODING_PCM_16BIT).build()
                ).setBufferSizeInBytes(
                    AudioRecord.getMinBufferSize(
                        16000,
                        AudioFormat.CHANNEL_IN_MONO,
                        AudioFormat.ENCODING_PCM_16BIT
                    )
                )
                .setAudioPlaybackCaptureConfig(
                    AudioPlaybackCaptureConfiguration.Builder(mediaProjection)
                        .addMatchingUsage(AudioAttributes.USAGE_MEDIA)
                        .addMatchingUsage(AudioAttributes.USAGE_GAME)
                        .build()
                ).build()
        )

        virtualDisplay.surface = sender?.getSurface()
    }

    private fun startNotification() {
        val builder = Notification.Builder(this.applicationContext)
        val nfIntent = Intent(this, MainActivity::class.java)
        builder.setContentIntent(
            PendingIntent.getActivity(
                this,
                0,
                nfIntent,
                PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT
            )
        ).setLargeIcon(
            BitmapFactory.decodeResource(
                this.resources,
                R.mipmap.sym_def_app_icon
            )
        )
            .setContentTitle("Screen recording")
            .setSmallIcon(R.mipmap.sym_def_app_icon)
            .setContentText("Recording screen......")
            .setWhen(System.currentTimeMillis())
        builder.setChannelId("MirrorVirtualDisplayServiceNotificationId")

        val notificationManager = getSystemService(NOTIFICATION_SERVICE) as NotificationManager
        val channel = NotificationChannel(
            "MirrorVirtualDisplayServiceNotificationId",
            "MirrorVirtualDisplayServiceNotificationName",
            NotificationManager.IMPORTANCE_LOW
        )
        notificationManager.createNotificationChannel(channel)

        val notification: Notification = builder.build()
        startForeground(110, notification)
    }

    override fun onBind(intent: Intent?): IBinder {
        return binder
    }

    override fun onDestroy() {
        super.onDestroy()
        mirror.release()
        sender?.release()
        mediaProjection.stop()
        virtualDisplay.release()
    }
}
