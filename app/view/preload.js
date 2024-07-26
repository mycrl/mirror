const { contextBridge, ipcRenderer } = require('electron/renderer')

contextBridge.exposeInMainWorld('electronAPI', {
    get_devices: (kind) => ipcRenderer.invoke('get-devices', kind),
    update_settings: (settings) => ipcRenderer.send('update-settings', settings),
})