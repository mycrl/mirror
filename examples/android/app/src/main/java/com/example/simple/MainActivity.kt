package com.example.simple

import android.Manifest
import android.app.Activity
import android.content.ComponentName
import android.content.Intent
import android.content.ServiceConnection
import android.media.projection.MediaProjectionManager
import android.os.Build
import android.os.Bundle
import android.os.IBinder
import android.util.Log
import android.view.Surface
import android.view.SurfaceHolder
import android.view.SurfaceView
import android.view.WindowInsets
import android.view.WindowManager
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.result.contract.ActivityResultContracts
import androidx.annotation.RequiresApi
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Button
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TextField
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.unit.dp
import androidx.compose.ui.viewinterop.AndroidView
import com.github.mycrl.hylarana.HylaranaStrategy
import com.github.mycrl.hylarana.HylaranaStrategyType

abstract class Observer {
    abstract fun OnConnect(strategy: HylaranaStrategy)

    abstract fun OnPublish()

    abstract fun OnSubscribe()

    abstract fun OnStop()
}

open class Layout : ComponentActivity() {
    private var observer: Observer? = null
    private var surfaceView: SurfaceView? = null
    private var clickStartHandler: (() -> Unit)? = null
    private var address by mutableStateOf("0.0.0.0:8080")
    private var state by mutableIntStateOf(State.New)

    class State {
        companion object {
            const val New = 0

            const val Connected = 1

            const val Publishing = 2

            const val Subscribeing = 3
        }
    }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        window.addFlags(WindowManager.LayoutParams.FLAG_KEEP_SCREEN_ON)
        setContent { CreateLayout() }
    }

    fun layoutSetObserver(observer: Observer) {
        this.observer = observer
    }

    fun layoutGetSurface(): Surface? {
        return surfaceView?.holder?.surface
    }

    fun layoutGetState(): Int {
        return state
    }

    fun layoutSetState(state: Int) {
        this.state = state
    }

    @RequiresApi(Build.VERSION_CODES.R)
    fun layoutStop() {
        state = State.Connected

        runOnUiThread {
            window.insetsController?.show(
                WindowInsets.Type.statusBars() or WindowInsets.Type.navigationBars()
            )
        }
    }

    @Composable
    private fun CreateLayout() {
        Surface(color = Color.Black) {
            AndroidView(
                factory = { ctx ->
                    val view =
                        SurfaceView(ctx).apply {
                            holder.addCallback(
                                object : SurfaceHolder.Callback {
                                    override fun surfaceCreated(holder: SurfaceHolder) {
                                        Log.i("simple", "create preview surface view.")
                                    }

                                    override fun surfaceChanged(
                                        holder: SurfaceHolder,
                                        format: Int,
                                        width: Int,
                                        height: Int
                                    ) {
                                        Log.i("simple", "preview surface view changed.")
                                    }

                                    override fun surfaceDestroyed(holder: SurfaceHolder) {
                                        Log.i("simple", "preview surface view destroyed.")
                                    }
                                }
                            )
                        }

                    surfaceView = view
                    view
                },
                modifier = Modifier.fillMaxSize(),
            )

            Box(modifier = Modifier.fillMaxSize()) {
                Column(
                    modifier =
                    Modifier.align(
                        if (state == State.Subscribeing) {
                            Alignment.BottomStart
                        } else {
                            Alignment.Center
                        }
                    ),
                    verticalArrangement =
                    if (state == State.Subscribeing) {
                        Arrangement.Bottom
                    } else {
                        Arrangement.Center
                    },
                    horizontalAlignment =
                    if (state == State.Subscribeing) {
                        Alignment.Start
                    } else {
                        Alignment.CenterHorizontally
                    },
                ) {
                    if (state == State.New) {
                        TextField(
                            value = address,
                            label = { Text(text = "Address") },
                            onValueChange = { address = it },
                            modifier = Modifier.padding(6.dp).width(320.dp),
                            shape = RoundedCornerShape(6.dp),
                        )
                    }

                    Row() {
                        when (state) {
                            State.New -> {
                                Button(
                                    onClick = {
                                        observer?.OnConnect(
                                            HylaranaStrategy(
                                                type = HylaranaStrategyType.DIRECT,
                                                addr = address
                                            )
                                        )
                                    },
                                    shape = RoundedCornerShape(8.dp),
                                    modifier = Modifier.width(100.dp),
                                ) {
                                    Text(text = "Direct")
                                }
                                Spacer(modifier = Modifier.width(10.dp))
                                Button(
                                    onClick = {
                                        observer?.OnConnect(
                                            HylaranaStrategy(
                                                type = HylaranaStrategyType.RELAY,
                                                addr = address
                                            )
                                        )
                                    },
                                    shape = RoundedCornerShape(8.dp),
                                    modifier = Modifier.width(100.dp),
                                ) {
                                    Text(text = "Relay")
                                }
                                Spacer(modifier = Modifier.width(10.dp))
                                Button(
                                    onClick = {
                                        observer?.OnConnect(
                                            HylaranaStrategy(
                                                type = HylaranaStrategyType.MULTICAST,
                                                addr = address
                                            )
                                        )
                                    },
                                    shape = RoundedCornerShape(8.dp),
                                    modifier = Modifier.width(100.dp),
                                ) {
                                    Text(text = "Multicast")
                                }
                            }
                            State.Connected -> {
                                Button(
                                    onClick = { ->
                                        state = State.Publishing
                                        observer?.OnPublish()
                                    },
                                    shape = RoundedCornerShape(8.dp),
                                    modifier = Modifier.width(140.dp),
                                ) {
                                    Text(text = "Publish")
                                }
                                Spacer(modifier = Modifier.width(20.dp))
                                Button(
                                    onClick = { ->
                                        state = State.Subscribeing
                                        observer?.OnSubscribe()
                                    },
                                    shape = RoundedCornerShape(8.dp),
                                    modifier = Modifier.width(140.dp),
                                ) {
                                    Text(text = "Subscribe")
                                }
                            }
                            else -> {
                                Button(
                                    onClick = { observer?.OnStop() },
                                    shape = RoundedCornerShape(8.dp),
                                    modifier = Modifier.width(140.dp),
                                ) {
                                    Text(text = "Stop")
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

open class Permissions : Layout() {
    private var callback: ((Intent?) -> Unit)? = null
    private var captureScreenIntent: Intent? = null
    private val captureScreenPermission =
        registerForActivityResult(ActivityResultContracts.StartActivityForResult()) { result ->
            if (result.resultCode == Activity.RESULT_OK && result.data != null) {
                Log.i("simple", "request screen capture permission done.")

                captureScreenIntent = result.data
                captureAudioPermission.launch(Manifest.permission.RECORD_AUDIO)
            } else {
                Log.e("simple", "failed to request screen capture permission.")

                callback?.let { it(null) }
            }
        }

    private val captureAudioPermission =
        registerForActivityResult(ActivityResultContracts.RequestPermission()) { isGranted ->
            callback?.let {
                it(
                    if (isGranted) {
                        Log.i("simple", "request audio capture permission done.")

                        captureScreenIntent
                    } else {
                        Log.e("simple", "failed request audio capture permission.")

                        null
                    }
                )
            }
        }

    fun requestPermissions() {
        captureScreenPermission.launch(
            (getSystemService(MEDIA_PROJECTION_SERVICE) as MediaProjectionManager)
                .createScreenCaptureIntent()
        )
    }

    fun registerPermissionsHandler(handler: (Intent?) -> Unit) {
        callback = handler
    }
}

@RequiresApi(Build.VERSION_CODES.R)
class MainActivity : Permissions() {
    private var simpleHylaranaService: Intent? = null
    private var simpleHylaranaServiceBinder: SimpleHylaranaServiceBinder? = null
    private val connection: ServiceConnection =
        object : ServiceConnection {
            override fun onServiceConnected(name: ComponentName?, service: IBinder?) {
                Log.i("simple", "service connected.")

                simpleHylaranaServiceBinder = service as SimpleHylaranaServiceBinder
                simpleHylaranaServiceBinder?.setObserver(
                    object : SimpleHylaranaServiceObserver() {
                        override fun onConnected() {
                            layoutSetState(State.Connected)
                        }

                        override fun onReceiverClosed() {
                            layoutStop()
                        }
                    }
                )

                layoutGetSurface()?.let { surface ->
                    simpleHylaranaServiceBinder?.setRenderSurface(surface)
                }
            }

            override fun onServiceDisconnected(name: ComponentName?) {
                Log.w("simple", "service disconnected.")
            }
        }

    init {
        registerPermissionsHandler { intent ->
            if (intent != null) {
                simpleHylaranaServiceBinder?.createSender(intent, resources.displayMetrics)
            }
        }

        layoutSetObserver(
            object : Observer() {
                override fun OnConnect(strategy: HylaranaStrategy) {
                    simpleHylaranaServiceBinder?.connect(strategy)
                }

                override fun OnPublish() {
                    requestPermissions()
                }

                override fun OnSubscribe() {
                    simpleHylaranaServiceBinder?.createReceiver()
                }

                override fun OnStop() {
                    val state = layoutGetState()
                    if (state == State.Publishing) {
                        simpleHylaranaServiceBinder?.stopSender()
                        layoutSetState(State.Connected)
                    } else {
                        simpleHylaranaServiceBinder?.stopReceiver()
                    }
                }
            }
        )
    }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        simpleHylaranaService = startSimpleHylaranaService()
    }

    override fun onDestroy() {
        super.onDestroy()
        stopService(simpleHylaranaService)
    }

    private fun startSimpleHylaranaService(): Intent {
        val intent = Intent(this, SimpleHylaranaService::class.java)
        bindService(intent, connection, BIND_AUTO_CREATE)

        Log.i("simple", "start simple hylarana service.")

        return intent
    }
}
