const { contextBridge, ipcRenderer } = require('electron/renderer')

contextBridge.exposeInMainWorld('electronAPI', {
    get_devices: (kind) => ipcRenderer.invoke('get-devices', kind),
    set_settings: (settings) => ipcRenderer.invoke('set-settings', settings),
    get_settings: () => ipcRenderer.invoke('get-settings'),
    create_sender: (device) => ipcRenderer.invoke('create-sender', device),
    close_sender: () => ipcRenderer.invoke('close-sender'),
    close: () => ipcRenderer.send('close'),
})