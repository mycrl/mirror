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
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.Icon
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.toArgb
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.unit.dp
import androidx.compose.ui.viewinterop.AndroidView

open class Layout : ComponentActivity() {
    private var surfaceView: SurfaceView? = null
    private var clickStartHandler: (() -> Unit)? = null
    private var buttonAlign by mutableStateOf(Alignment.Center)
    private var icon by mutableIntStateOf(R.drawable.cell_tower)
    private var state: Int = State.New
    private var socketaddr: String? = null

    class State {
        companion object {
            const val New = 0;
            const val Started = 1;
            const val Receiving = 2;
            const val StopReceiving = 3;
        }
    }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        window.addFlags(WindowManager.LayoutParams.FLAG_KEEP_SCREEN_ON)
        setContent {
            CreateLayout()
        }
    }

    fun layoutGetPeerAddr(): String? {
        return socketaddr
    }

    fun layoutGetSurface(): Surface? {
        return surfaceView?.holder?.surface
    }

    fun layoutGetState(): Int {
        return state
    }

    fun layoutRegisterClickStart(handler: () -> Unit) {
        clickStartHandler = handler
    }

    fun layoutChangeReceived(addr: String?) {
        if (addr != null) {
            socketaddr = addr
        }

        state = State.Receiving
        icon = R.drawable.stop_circle
        buttonAlign = Alignment.BottomStart

        runOnUiThread {
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.R) {
                window.insetsController?.hide(WindowInsets.Type.statusBars() or WindowInsets.Type.navigationBars())
            } else {
                window.decorView.systemUiVisibility = (View.SYSTEM_UI_FLAG_FULLSCREEN
                        or View.SYSTEM_UI_FLAG_HIDE_NAVIGATION
                        or View.SYSTEM_UI_FLAG_IMMERSIVE_STICKY)
                window.addFlags(WindowManager.LayoutParams.FLAG_FULLSCREEN)
            }
        }
    }

    fun layoutChangeStarted() {
        state = State.Started
        icon = R.drawable.wifi_tethering
        buttonAlign = Alignment.Center
    }

    fun layoutStopReceiving() {
        state = State.StopReceiving
        icon = R.drawable.link
        buttonAlign = Alignment.Center
    }

    fun layoutChangeReset() {
        state = State.New
        icon = R.drawable.cell_tower
        buttonAlign = Alignment.Center

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
                Button(
                    onClick = {
                        clickStartHandler?.let { it() }
                    },
                    modifier = Modifier
                        .padding(20.dp)
                        .align(buttonAlign),
                    shape = RoundedCornerShape(8.dp),
                    colors = ButtonDefaults.buttonColors(containerColor = Color(0xFF0C9CF8)),
                ) {
                    Icon(
                        painter = painterResource(id = icon),
                        contentDescription = "Cell Tower",
                        tint = Color.White
                    )

                    if (state != State.New) {
                        Spacer(modifier = Modifier.width(15.dp))
                        Text(
                            text = when (state) {
                                State.StopReceiving -> "$socketaddr"
                                State.Started -> "Screen casting, Click to stop"
                                else -> "Receiving screen casting, Click to stop"
                            }, modifier = Modifier
                        )
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
    private var simpleMirrorService: Intent? = null
    private var simpleMirrorServiceBinder: SimpleMirrorServiceBinder? = null
    private val connection: ServiceConnection = object : ServiceConnection {
        override fun onServiceConnected(name: ComponentName?, service: IBinder?) {
            Log.i("simple", "service connected.")

            simpleMirrorServiceBinder = service as SimpleMirrorServiceBinder
            simpleMirrorServiceBinder?.registerReceivedHandler { id, addr ->
                Log.i("simple", "start receiving sender stream. id=$id, addr=$addr")

                layoutChangeReceived(addr)
            }

            simpleMirrorServiceBinder?.registerReceivedReleaseHandler { id, addr ->
                Log.i("simple", "receiver is released. id=$id, ip=$addr")

                layoutStopReceiving()
            }

            layoutGetSurface()?.let { surface ->
                simpleMirrorServiceBinder?.setRenderSurface(surface)
            }
        }

        override fun onServiceDisconnected(name: ComponentName?) {
            Log.w("simple", "service disconnected.")
        }
    }

    init {
        registerPermissionsHandler { intent ->
            if (intent != null) {
                simpleMirrorServiceBinder?.createSender(intent, resources.displayMetrics)
                layoutChangeStarted()
            }
        }

        layoutRegisterClickStart {
            val state = layoutGetState()
            Log.i("simple", "click start button. state=${state}")

            when (state) {
                State.New -> requestPermissions()
                State.StopReceiving -> {
                    simpleMirrorServiceBinder?.createReceiver(layoutGetPeerAddr()!!)
                    layoutChangeReceived(null)
                }

                State.Receiving -> {
                    simpleMirrorServiceBinder?.stopReceiver()
                }

                State.Started -> {
                    simpleMirrorServiceBinder?.stopSender()
                    layoutChangeReset()
                }
            }
        }
    }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        simpleMirrorService = startSimpleMirrorService()
    }

    override fun onDestroy() {
        super.onDestroy()
        stopService(simpleMirrorService)
    }

    private fun startSimpleMirrorService(): Intent {
        val intent = Intent(this, SimpleMirrorService::class.java)
        bindService(intent, connection, BIND_AUTO_CREATE)

        Log.i("simple", "start simple mirror service.")

        return intent
    }
}