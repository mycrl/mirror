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
import android.view.View
import android.view.WindowInsets
import android.view.WindowManager
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.result.contract.ActivityResultContracts
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

abstract class Observer {
    abstract fun OnConnect(server: String);
    abstract fun OnPublish(id: Int);
    abstract fun OnSubscribe(id: Int);
    abstract fun OnStop();
    abstract fun SetMulticast(isMulticast: Boolean);
}

open class Layout : ComponentActivity() {
    private var observer: Observer? = null
    private var surfaceView: SurfaceView? = null
    private var clickStartHandler: (() -> Unit)? = null
    private var server by mutableStateOf("192.168.2.129:8088")
    private var id by mutableStateOf("0")
    private var state by mutableIntStateOf(State.New)
    private var isMulticast by mutableIntStateOf(0)

    class State {
        companion object {
            const val New = 0;
            const val Connected = 1;
            const val Publishing = 2;
            const val Subscribeing = 3;
        }
    }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        window.addFlags(WindowManager.LayoutParams.FLAG_KEEP_SCREEN_ON)
        setContent {
            CreateLayout()
        }
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

    fun layoutStop() {
        state = State.Connected
        isMulticast = 0

        runOnUiThread {
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.R) {
                window.insetsController?.show(WindowInsets.Type.statusBars() or WindowInsets.Type.navigationBars())
            } else {
                window.decorView.systemUiVisibility = View.SYSTEM_UI_FLAG_VISIBLE
                window.clearFlags(WindowManager.LayoutParams.FLAG_FULLSCREEN)
            }
        }
    }

    @Composable
    private fun CreateLayout() {
        Surface(color = Color.Black) {
            AndroidView(
                factory = { ctx ->
                    val view = SurfaceView(ctx).apply {
                        holder.addCallback(object : SurfaceHolder.Callback {
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
                        })
                    }

                    surfaceView = view
                    view
                },
                modifier = Modifier
                    .fillMaxSize(),
            )

            Box(modifier = Modifier.fillMaxSize()) {
                Column(
                    modifier = Modifier.align(
                        if (state == State.Subscribeing) {
                            Alignment.BottomStart
                        } else {
                            Alignment.Center
                        }
                    ),
                    verticalArrangement = if (state == State.Subscribeing) {
                        Arrangement.Bottom
                    } else {
                        Arrangement.Center
                    },
                    horizontalAlignment = if (state == State.Subscribeing) {
                        Alignment.Start
                    } else {
                        Alignment.CenterHorizontally
                    },
                ) {
                    if (state == State.New) {
                        TextField(
                            value = server,
                            label = { Text(text = "Server Address") },
                            onValueChange = { server = it },
                            modifier = Modifier
                                .padding(6.dp)
                                .width(300.dp),
                            shape = RoundedCornerShape(6.dp),
                        )
                    }

                    if (state == State.Connected) {
                        TextField(
                            value = id,
                            label = { Text(text = "Stream ID") },
                            onValueChange = { id = it },
                            modifier = Modifier
                                .padding(6.dp)
                                .width(300.dp),
                            shape = RoundedCornerShape(6.dp),
                        )
                    }

                    Row() {
                        if (state == State.New) {
                            Button(
                                onClick = { observer?.OnConnect(server) },
                                shape = RoundedCornerShape(8.dp),
                                modifier = Modifier.width(300.dp),
                            ) {
                                Text(text = "Connect")
                            }
                        } else if (state == State.Connected) {
                            Button(
                                onClick = { ->
                                    state = State.Publishing
                                    observer?.OnPublish(id.toInt())
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
                                    observer?.OnSubscribe(id.toInt())
                                },
                                shape = RoundedCornerShape(8.dp),
                                modifier = Modifier.width(140.dp),
                            ) {
                                Text(text = "Subscribe")
                            }
                        } else {
                            Button(
                                onClick = { observer?.OnStop() },
                                shape = RoundedCornerShape(8.dp),
                                modifier = Modifier.width(140.dp),
                            ) {
                                Text(text = "Stop")
                            }

                            if (state == State.Publishing) {
                                Spacer(modifier = Modifier.width(20.dp))
                                Button(
                                    onClick = { ->
                                        isMulticast = if (isMulticast == 0) {
                                            1
                                        } else {
                                            0
                                        }

                                        observer?.SetMulticast(isMulticast != 0)
                                    },
                                    shape = RoundedCornerShape(8.dp),
                                    modifier = Modifier.width(150.dp),
                                ) {
                                    Text(
                                        text = if (isMulticast == 0) {
                                            "Enable Multicast"
                                        } else {
                                            "Disable Multicast"
                                        }
                                    )
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
            (getSystemService(MEDIA_PROJECTION_SERVICE) as MediaProjectionManager).createScreenCaptureIntent()
        )
    }

    fun registerPermissionsHandler(handler: (Intent?) -> Unit) {
        callback = handler
    }
}

class MainActivity : Permissions() {
    private var simpleHylaranaService: Intent? = null
    private var simpleHylaranaServiceBinder: SimpleHylaranaServiceBinder? = null
    private val connection: ServiceConnection = object : ServiceConnection {
        override fun onServiceConnected(name: ComponentName?, service: IBinder?) {
            Log.i("simple", "service connected.")

            simpleHylaranaServiceBinder = service as SimpleHylaranaServiceBinder
            simpleHylaranaServiceBinder?.setObserver(object : SimpleHylaranaServiceObserver() {
                override fun onConnected() {
                    layoutSetState(State.Connected)
                }

                override fun onReceiverClosed() {
                    layoutStop()
                }
            })

            layoutGetSurface()?.let { surface ->
                simpleHylaranaServiceBinder?.setRenderSurface(surface)
            }
        }

        override fun onServiceDisconnected(name: ComponentName?) {
            Log.w("simple", "service disconnected.")
        }
    }

    init {
        var senderId = 0

        registerPermissionsHandler { intent ->
            if (intent != null) {
                simpleHylaranaServiceBinder?.createSender(intent, resources.displayMetrics, senderId)
            }
        }

        layoutSetObserver(object : Observer() {
            override fun OnConnect(server: String) {
                simpleHylaranaServiceBinder?.connect(server)
            }

            override fun OnPublish(id: Int) {
                senderId = id
                requestPermissions()
            }

            override fun OnSubscribe(id: Int) {
                simpleHylaranaServiceBinder?.createReceiver(id)
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

            override fun SetMulticast(isMulticast: Boolean) {
                simpleHylaranaServiceBinder?.setMulticast(isMulticast)
            }
        })
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
