import { contextBridge, ipcRenderer } from "electron/renderer";
import { MirrorSourceDescriptor, MirrorSourceType } from "mirror-napi";

contextBridge.exposeInMainWorld("electronAPI", {
    getSources: (kind: MirrorSourceType) => ipcRenderer.invoke("get-sources", kind),
    setSettings: (settings: any) => ipcRenderer.invoke("set-settings", settings),
    getSettings: () => ipcRenderer.invoke("get-settings"),
    createSender: (device: { video?: MirrorSourceDescriptor; audio?: MirrorSourceDescriptor }) =>
        ipcRenderer.invoke("create-sender", device),
    closeSender: () => ipcRenderer.invoke("close-sender"),
    close: () => ipcRenderer.send("close"),
});
