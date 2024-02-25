package com.example.simple

import android.Manifest
import android.app.Activity
import android.content.ComponentName
import android.content.Intent
import android.content.ServiceConnection
import android.media.projection.MediaProjectionManager
import android.os.Bundle
import android.os.IBinder
import android.util.Log
import android.view.Surface
import android.view.SurfaceHolder
import android.view.SurfaceView
import android.view.View
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Button
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.compose.ui.viewinterop.AndroidView

open class Layout : ComponentActivity() {
    private var surfaceView: SurfaceView? = null
    private var clickStartHandler: (() -> Unit)? = null
    private var buttonAlign by mutableStateOf(Alignment.Center)
    private var buttonText by mutableStateOf("Start")
    private var state: Int = State.New

    class State {
        companion object {
            const val New = 0;
            const val Started = 1;
            const val Receiving = 2;
        }
    }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        setContent {
            CreateLayout()
        }
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

    fun layoutChangeReceived() {
        state = State.Receiving
        buttonText = "Receiving... Stop"
        buttonAlign = Alignment.BottomStart

        runOnUiThread {
            surfaceView?.let { view ->
                view.visibility = View.VISIBLE
            }
        }
    }

    fun layoutChangeStarted() {
        state = State.Started
        buttonText = "Working... Stop"
        buttonAlign = Alignment.Center

        runOnUiThread {
            surfaceView?.let { view ->
                view.visibility = View.INVISIBLE
            }
        }
    }

    fun layoutChangeReset() {
        state = State.New
        buttonText = "Start"
        buttonAlign = Alignment.Center

        runOnUiThread {
            surfaceView?.let { view ->
                view.visibility = View.INVISIBLE
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

                    view.visibility = View.INVISIBLE
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
                    shape = RoundedCornerShape(8.dp)
                ) {
                    Text(
                        text = buttonText,
                        color = Color.White,
                        fontSize = 20.sp,
                        fontWeight = FontWeight.Bold
                    )
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
            simpleMirrorServiceBinder?.registerReceivedHandler { id, ip ->
                Log.i("simple", "start receiving sender stream. id=${id}, ip=${ip}")

                layoutChangeReceived()
            }

            simpleMirrorServiceBinder?.registerReceivedReleaseHandler { id, ip ->
                Log.i("simple", "receiver is released. id=${id}, ip=${ip}")

                layoutChangeReset()
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
                simpleMirrorServiceBinder?.startup(intent, resources.displayMetrics)
                layoutChangeStarted()
            }
        }

        layoutRegisterClickStart {
            val state = layoutGetState()
            Log.i("simple", "click start button. state=${state}")

            if (state == State.New) {
                requestPermissions()
            } else {
                layoutChangeReset()
                when (state) {
                    State.Receiving -> simpleMirrorServiceBinder?.stopReceiver()
                    State.Started -> simpleMirrorServiceBinder?.stopSender()
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