const { contextBridge, ipcRenderer } = require('electron/renderer')

contextBridge.exposeInMainWorld('electronAPI', {
    getSources: (kind) => ipcRenderer.invoke('get-sources', kind),
    setSettings: (settings) => ipcRenderer.invoke('set-settings', settings),
    getSettings: () => ipcRenderer.invoke('get-settings'),
    createSender: (device) => ipcRenderer.invoke('create-sender', device),
    closeSender: () => ipcRenderer.invoke('close-sender'),
    close: () => ipcRenderer.send('close'),
})
