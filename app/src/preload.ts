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

contextBridge.exposeInMainWorld("types", {
    MirrorSourceType: {
        /**
         * Camera or video capture card and other devices (and support virtual
         * camera)
         */
        Camera: 0,
        /**
         * The desktop or monitor corresponds to the desktop in the operating
         * system.
         */
        Screen: 1,
        /** Audio input and output devices. */
        Audio: 2,
    },
    MirrorVideoDecoderType: {
        /** h264 (software) */
        H264: 0,
        /** d3d11va */
        D3D11: 1,
        /** h264_qsv */
        Qsv: 2,
        /** h264_cvuid */
        Cuda: 3,
        /** video tool box */
        VideoToolBox: 4,
    },
    MirrorVideoEncoderType: {
        /** libx264 (software) */
        X264: 0,
        /** h264_qsv */
        Qsv: 1,
        /** h264_nvenc */
        Cuda: 2,
        /** video tool box */
        VideoToolBox: 3,
    },
});
