const { app, screen, BrowserWindow, Tray, nativeImage, ipcMain } = require('electron')
const { MirrorService } = require('mirror')
const { join } = require('path')

const tray = new Tray(nativeImage.createFromPath('./icon/light.png'))
const display = screen.getPrimaryDisplay()
const window = new BrowserWindow({
    x: display.size.width - 220,
    y: display.size.height - 440,
    width: 200,
    height: 400,
    frame: false,
    resizable: false,
    movable: false,
    minimizable: false,
    maximizable: false,
    alwaysOnTop: true,
    fullscreenable: false,
    transparent: true,
    webPreferences: {
        devTools: true,
        preload: join(__dirname, '../view/preload.js'),
    },
})

window.loadFile(join(__dirname, '../view/index.html'))
window.webContents.openDevTools({
    mode: 'detach',
})

tray.setTitle('mirror')
tray.setToolTip('Cross-platform screen projection')

const mirror = new MirrorService()
const capture = mirror.create_capture_service()
const State = {
    channel: 0,
    sender: null,
    receiver: null,
    is_capture: false,
}

ipcMain.on('create-sender', (_event) =>
{
    State.sender = mirror.create_sender(State.channel)
})

ipcMain.on('stop-sender', (_event) =>
{
    if (State.sender != null)
    {
        State.sender.close()
        State.sender = null
    }
})

ipcMain.on('update-settings', (_event, { id, ...settings }) =>
{
    State.channel = id
    if (State.receiver != null)
    {
        State.receiver.close()
        State.receiver = null
    }

    mirror.quit()
    mirror.init(settings)
    State.receiver = mirror.create_receiver(id, () =>
    {
        State.receiver.close()
        State.receiver = null
    })
})

ipcMain.handle('get-devices', (_event, kind) =>
{
    if (!State.is_capture)
    {
        capture.start_capture()
        State.is_capture = true
    }

    return capture.get_devices(kind)
})

app.on('window-all-closed', () =>
{
    app.exit()
})