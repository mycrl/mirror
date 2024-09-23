import { app, screen, BrowserWindow, Tray, nativeImage, ipcMain, Menu } from 'electron'
import { MirrorService } from 'mirror-napi'
import { join } from 'node:path'
import * as fs from 'node:fs'

const Config = {
    path: './settings.json',
    get()
    {
        if (!fs.existsSync(this.path))
        {
            fs.writeFileSync(this.path, '{}')
        }

        return JSON.parse(fs.readFileSync(this.path, 'utf8') || "{}")
    },
    set(settings)
    {
        fs.writeFileSync(this.path, JSON.stringify(settings, null, 4))
    }
}

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
tray.setContextMenu(Menu.buildFromTemplate([
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

const Notify = (info: string) =>
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

const Log = (level: string, message: string) => 
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
    is_init: false,
}

ipcMain.handle('create-sender', (_event, device) =>
{
    Log('info', 'ipc create sender event')
    if (!State.is_init)
    {
        return
    }

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
    if (!State.is_init)
    {
        return
    }

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

ipcMain.handle('get-settings', () =>
{
    return Config.get()
})

ipcMain.handle('set-settings', (_event, { id, ...settings }) =>
{
    Log('info', 'ipc update settings event')

    Config.set({
        ...settings,
        id,
    })

    State.channel = id
    if (State.receiver)
    {
        Log('info', 'receiver is exists, close receiver')

        State.receiver.close()
        State.receiver = null
    }

    Log('info', 'rebuild mirror env')

    try
    {
        State.is_init = false

        mirror.quit()
        mirror.init(settings)

        State.is_init = true
    }
    catch (e)
    {
        Log('error', e)
        Notify('Initialization failed due to an error in setting parameters!')

        State.is_init = false
        return
    }

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

    if (!State.is_init)
    {
        return
    }

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
