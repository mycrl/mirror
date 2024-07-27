const { app, screen, BrowserWindow, Tray, nativeImage, ipcMain, Menu } = require('electron')
const { MirrorService } = require('mirror')
const { join } = require('path')

const tray = new Tray(nativeImage.createFromPath(join(__dirname, '../icon/light.png')))
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
        preload: join(__dirname, '../view/preload.js'),
    },
})

window.loadFile(join(__dirname, '../view/index.html'))

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
    capture.set_input_device(device)
    State.sender = mirror.create_sender(State.channel, () =>
    {
        Notify('Screen projection has stopped')
    })
})

ipcMain.handle('close-sender', async (_event) =>
{
    if (State.sender != null)
    {
        State.sender.close()
        State.sender = null
    }

    capture.stop()
    State.is_capture = false
})

ipcMain.handle('update-settings', (_event, { id, ...settings }) =>
{
    State.channel = id
    if (State.receiver != null)
    {
        State.receiver.close()
        State.receiver = null
    }

    mirror.quit()
    mirror.init(settings)
    State.receiver = mirror.create_receiver(id, {
        width: display.workAreaSize.width,
        height: display.workAreaSize.height,
    }, () =>
    {
        State.receiver.close()
        State.receiver = null

        Notify('Other devices have turned off screen projection')
    })
})

ipcMain.handle('get-devices', (_event, kind) =>
{
    if (!State.is_capture)
    {
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