const { app, screen, BrowserWindow, Tray, nativeImage, ipcMain, Menu } = require('electron')
const { MirrorService } = require('mirror-napi')
const { join } = require('path')

const tray = new Tray(nativeImage.createFromPath(join(__dirname, '../icon.ico')))
const display = screen.getPrimaryDisplay()
const window = new BrowserWindow({
    x: display.workAreaSize.width - 210,
    y: display.workAreaSize.height - 420,
    width: 200,
    height: 420,
    frame: false,
    resizable: false,
    movable: false,
    minimizable: false,
    maximizable: false,
    alwaysOnTop: true,
    fullscreenable: false,
    transparent: true,
    skipTaskbar: true,
    show: false,
    webPreferences: {
        preload: join(__dirname, './view/preload.js'),
    },
})

window.loadFile(join(__dirname, './view/index.html'))

tray.setTitle('mirror')
tray.setToolTip('service is running')
tray.setContextMenu(new Menu.buildFromTemplate([
    {
        label: 'Open DevTools',
        click: () =>
        {
            window.webContents.openDevTools({
                mode: 'detach',
            })
        }
    },
    {
        label: 'Exit',
        click: () =>
        {
            app.exit()
        }
    },
]))

const Notify = (info) =>
{
    tray.displayBalloon({
        iconType: 'info',
        title: 'Mirror - Cross-platform screen casting',
        content: info,
    })

    setTimeout(() =>
    {
        tray.removeBalloon()
    }, 3000)
}

const Log = (level, message) => 
{
    console.log(`-> ELECTRON: [${level}] - ${message}`)
}

tray.on('double-click', (_event, bounds) =>
{
    window.setPosition(bounds.x - 90, display.workAreaSize.height - 420)
    window.show()
})

Notify('The service is running in the background. Double-click the icon to expand it.')

const mirror = new MirrorService()
const capture = mirror.create_capture_service()
const State = {
    channel: 0,
    sender: null,
    receiver: null,
    is_capture: false,
}

ipcMain.handle('create-sender', (_event, device) =>
{
    Log('info', 'ipc create sender event')

    if (State.receiver)
    {
        Log('info', 'receiver is exists, close receiver')

        State.receiver.close()
        State.receiver = null
    }

    if (!State.sender)
    {
        Log('info', 'create sender')

        capture.set_input_device(device)
        State.sender = mirror.create_sender(State.channel, () =>
        {
            Log('info', 'sender close callback')

            Notify('Screen projection has stopped')
        })
    }
    else
    {
        Log('error', 'sender is exists')
    }
})

ipcMain.handle('close-sender', async (_event) =>
{
    Log('info', 'ipc close sender event')

    if (State.sender)
    {
        Log('info', 'close sender')

        State.sender.close()
        State.sender = null
    }

    Log('info', 'stop capture')

    capture.stop()
    State.is_capture = false

    if (!State.receiver)
    {
        Log('info', 'receiver not exists, create receiver')

        State.receiver = mirror.create_receiver(State.channel, () =>
        {
            Log('info', 'receiver close callback')

            if (State.receiver)
            {
                State.receiver.close()
                State.receiver = null
            }

            Notify('Other devices have turned off screen projection')
        })
    }
    else
    {
        Log('warn', 'receiver is exists, skip')
    }
})

ipcMain.handle('update-settings', (_event, { id, ...settings }) =>
{
    Log('info', 'ipc update settings event')

    State.channel = id
    if (State.receiver)
    {
        Log('info', 'receiver is exists, close receiver')

        State.receiver.close()
        State.receiver = null
    }

    Log('info', 'rebuild mirror env')

    mirror.quit()
    mirror.init(settings)

    if (!State.receiver)
    {
        Log('info', 'receiver not exists, create receiver')

        State.receiver = mirror.create_receiver(id, () =>
        {
            Log('info', 'receiver close callback')

            if (State.receiver)
            {
                State.receiver.close()
                State.receiver = null
            }

            Notify('Other devices have turned off screen projection')
        })
    }
    else
    {
        Log('warn', 'receiver is exists, skip')
    }
})

ipcMain.handle('get-devices', (_event, kind) =>
{
    Log('info', 'ipc get devices event')

    if (!State.is_capture)
    {
        Log('info', 'not start capture, start capture')

        capture.start()
        State.is_capture = true
    }

    return capture.get_devices(kind)
})

ipcMain.on('close', () =>
{
    window.hide()
})

app.on('window-all-closed', () =>
{
    app.exit()
})
