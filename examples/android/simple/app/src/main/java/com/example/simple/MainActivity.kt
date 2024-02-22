package com.example.simple

import android.Manifest
import android.app.Activity
import android.content.ComponentName
import android.content.Intent
import android.content.ServiceConnection
import android.graphics.Color
import android.media.projection.MediaProjectionManager
import android.os.Bundle
import android.os.IBinder
import android.util.Log
import android.view.Gravity
import android.view.SurfaceHolder
import android.view.SurfaceView
import android.view.View
import android.widget.Button
import android.widget.FrameLayout
import androidx.activity.ComponentActivity
import androidx.activity.result.contract.ActivityResultContracts
import androidx.core.app.ActivityCompat
import java.io.File
import java.io.FileOutputStream

class MainActivity : ComponentActivity() {
    private var screenCaptureService: Intent? = null
    private var screenCaptureServiceBinder: ScreenCaptureServiceBinder? = null
    private lateinit var surfaceView: SurfaceView
    private lateinit var button: Button
    private var permissionLauncherData: Intent? = null

    private val connection: ServiceConnection = object : ServiceConnection {
        override fun onServiceConnected(name: ComponentName?, service: IBinder?) {
            screenCaptureServiceBinder = service as ScreenCaptureServiceBinder
            screenCaptureServiceBinder?.setRenderSurface(surfaceView.holder.surface)
        }

        override fun onServiceDisconnected(name: ComponentName?) {

        }
    }

    override fun onRequestPermissionsResult(
        requestCode: Int,
        permissions: Array<out String>,
        grantResults: IntArray
    ) {
        super.onRequestPermissionsResult(requestCode, permissions, grantResults)
        screenCaptureServiceBinder?.startup(permissionLauncherData!!)
    }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)

        val intent = Intent(this, ScreenCaptureService::class.java)
        bindService(intent, connection, BIND_AUTO_CREATE)
        screenCaptureService = intent

        val mediaProjectionManager =
            getSystemService(MEDIA_PROJECTION_SERVICE) as MediaProjectionManager
        val permissionLauncher =
            registerForActivityResult(ActivityResultContracts.StartActivityForResult()) { result ->
                if (result.resultCode == Activity.RESULT_OK) {
                    val data: Intent? = result.data
                    if (data != null) {
                        permissionLauncherData = data
                        if (checkSelfPermission(android.Manifest.permission.RECORD_AUDIO) != android.content.pm.PackageManager.PERMISSION_GRANTED) {
                            requestPermissions(
                                arrayOf(Manifest.permission.RECORD_AUDIO),
                                123
                            )
                        } else {
                            screenCaptureServiceBinder?.startup(permissionLauncherData!!)
                        }
                    }
                } else {
                    // panic!
                }
            }

        surfaceView = SurfaceView(this)
        button = Button(this)

        button.text = "开始投屏"
        button.setBackgroundColor(Color.BLUE)
        button.setTextColor(Color.WHITE)

        val buttonLayoutParams = FrameLayout.LayoutParams(
            FrameLayout.LayoutParams.WRAP_CONTENT,
            FrameLayout.LayoutParams.WRAP_CONTENT
        )

        buttonLayoutParams.gravity = Gravity.CENTER
        button.layoutParams = buttonLayoutParams

        val projectionIntent = mediaProjectionManager.createScreenCaptureIntent()
        button.setOnClickListener(object : View.OnClickListener {
            override fun onClick(v: View?) {
                projectionIntent.let {
                    permissionLauncher.launch(it)
                }
            }
        })

        surfaceView.holder.addCallback(object : SurfaceHolder.Callback {
            override fun surfaceCreated(holder: SurfaceHolder) {

            }

            override fun surfaceChanged(
                holder: SurfaceHolder,
                format: Int,
                width: Int,
                height: Int
            ) {

            }

            override fun surfaceDestroyed(holder: SurfaceHolder) {

            }
        })

        val frameLayout = FrameLayout(this)
        frameLayout.setBackgroundColor(Color.BLACK)
        frameLayout.addView(surfaceView)
        frameLayout.addView(button)
        setContentView(frameLayout)
    }

    override fun onDestroy() {
        super.onDestroy()
        stopService(screenCaptureService)
    }
}