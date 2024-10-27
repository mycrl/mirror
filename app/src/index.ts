import { app, screen, BrowserWindow, Tray, nativeImage, ipcMain, Menu, BaseWindow } from "electron";
import {
    MirrorBackend,
    MirrorReceiverService,
    MirrorSenderService,
    MirrorService,
    shutdown,
    MirrorSourceDescriptor,
    MirrorSourceType,
    startup,
    MirrorVideoDecoderType,
    MirrorVideoEncoderType,
    Events,
    MirrorNativeWindowHandle,
} from "mirror-napi";
import { join } from "node:path";
import * as fs from "node:fs";
import { endianness } from "node:os";

const USER_DATA = app.getPath("userData");
const CONFIG_PATH = join(USER_DATA, "./configure");

if (!fs.existsSync(CONFIG_PATH)) {
    fs.writeFileSync(
        CONFIG_PATH,
        JSON.stringify(
            {
                channel: 0,
                server: "127.0.0.1:8080",
                multicast: "239.0.0.1",
                mtu: 1400,
                decoder: MirrorVideoDecoderType.D3D11,
                encoder: MirrorVideoEncoderType.Qsv,
                frameRate: 24,
                width: 1280,
                height: 720,
                bitRate: 500 * 1024 * 8,
                keyFrameInterval: 20,
            },
            null,
            4
        )
    );
}

let Config: {
    channel: number;
    server: string;
    multicast: string;
    mtu: number;
    decoder: MirrorVideoDecoderType;
    encoder: MirrorVideoEncoderType;
    frameRate: number;
    width: number;
    height: number;
    bitRate: number;
    keyFrameInterval: number;
} = new Proxy(JSON.parse(fs.readFileSync(CONFIG_PATH, "utf-8")), {
    get: (target: any, key, _) => {
        return target[key];
    },
    set: (target: any, key, value, _) => {
        target[key] = value;

        fs.writeFileSync(CONFIG_PATH, JSON.stringify(target, null, 4));
        return true;
    },
});

const display = screen.getPrimaryDisplay();
const trayWindow = new BrowserWindow({
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
        preload: join(__dirname, "./preload.js"),
    },
});

// trayWindow.loadFile(join(__dirname, "../view/index.html"));
trayWindow.loadURL("http://localhost:3000");

const baseWindow = new BaseWindow({
    width: 1280,
    height: 720,
    // resizable: false,
    // movable: false,
    // minimizable: false,
    // maximizable: false,
    // alwaysOnTop: false,
    // fullscreenable: true,
    // fullscreen: true,
    show: false,
    frame: false,
    // autoHideMenuBar: true,
    transparent: false,
});

const logo =
    process.platform == "darwin"
        ? nativeImage.createFromPath(join(__dirname, "../logoTemplate.png"))
        : nativeImage.createFromPath(join(__dirname, "../logo.ico"));

if (process.platform == "darwin") {
    logo.setTemplateImage(true);
}

const tray = new Tray(logo);

if (process.platform == "win32") {
    tray.setTitle("mirror");
}

tray.setToolTip("service is running");
tray.setContextMenu(
    Menu.buildFromTemplate([
        {
            label: "打开主界面",
            click: () => {
                tray.emit("double-click", {}, { x: 0, y: 0 });
            },
        },
        {
            label: "切换开发人员工具",
            click: () => {
                trayWindow.webContents.openDevTools({
                    mode: "detach",
                });
            },
        },
        {
            label: "退出",
            click: () => {
                app.exit();
            },
        },
    ])
);

function Notify(level: "info" | "warning" | "error", info: string) {
    tray.displayBalloon({
        title: "mirror",
        content: info,
        iconType: level,
        largeIcon: false,
        icon: logo,
    });

    setTimeout(() => {
        tray.removeBalloon();
    }, 5000);
}

function Log(level: string, ...args: any[]) {
    console.log(`[${level.toUpperCase()}] - (electron) - `, ...args);
}

tray.on("double-click", (_event, bounds) => {
    trayWindow.setPosition(bounds.x - 90, display.workAreaSize.height - 420);
    trayWindow.show();
});

Notify("info", "The service is running in the background. Double-click the icon to expand it.");

try {
    Log("info", `startup mirror, user data path=${USER_DATA}`);

    startup(USER_DATA);
} catch (e: any) {
    Log("error", e);
    Notify("error", e.message);

    process.exit(-1);
}

let mirror: MirrorService | null = null;
let sender: MirrorSenderService | null = null;
let receiver: MirrorReceiverService | null = null;

function closeMirror() {
    if (mirror != null) {
        mirror.destroy();
        mirror = null;
    }
}

function createMirror(settings: typeof Config): boolean {
    try {
        closeMirror();

        const { width, height } = display.size;
        const handle = baseWindow.getNativeWindowHandle();

        let hwnd: MirrorNativeWindowHandle = {};

        if (process.platform == "win32") {
            hwnd.windows = {
                hwnd: endianness() == "LE" ? handle.readBigInt64LE() : handle.readBigInt64BE(),
                width: 1280,
                height: 720,
            };
        } else if (process.platform == "darwin") {
            hwnd.macos = {
                nsView: endianness() == "LE" ? handle.readBigInt64LE() : handle.readBigInt64BE(),
                width: 1280,
                height: 720,
            };
        }

        mirror = new MirrorService({
            backend: MirrorBackend.WebGPU,
            multicast: settings.multicast,
            server: settings.server,
            mtu: settings.mtu,
            windowHandle: hwnd,
        });
    } catch (e: any) {
        Log("error", e);
        Notify("error", e.message);

        return false;
    }

    return true;
}

function closeSender() {
    if (sender != null) {
        sender.destroy();
        sender = null;
    }
}

function createSender(sources: {
    video?: MirrorSourceDescriptor;
    audio?: MirrorSourceDescriptor;
}): boolean {
    if (mirror == null) {
        return false;
    }

    try {
        closeSender();

        sender = mirror.createSender(
            Config.channel,
            {
                multicast: false,
                video: sources.video
                    ? {
                          source: sources.video,
                          settings: {
                              codec: Config.encoder,
                              frameRate: Config.frameRate,
                              width: Config.width,
                              height: Config.height,
                              bitRate: Config.bitRate,
                              keyFrameInterval: Config.keyFrameInterval,
                          },
                      }
                    : undefined,
                audio: sources.audio
                    ? {
                          source: sources.audio,
                          settings: {
                              sampleRate: 48000,
                              bitRate: 64000,
                          },
                      }
                    : undefined,
            },
            (event) => {
                if (event == Events.Closed) {
                    Log("info", "sender close callback");
                    Notify("info", "Screen projection has stopped");

                    closeSender();
                }
            }
        );
    } catch (e: any) {
        Log("error", e);
        Notify("error", e.message);

        return false;
    }

    return true;
}

function closeReceiver() {
    if (receiver != null) {
        receiver.destroy();
        receiver = null;
    }
}

function createReceiver(): boolean {
    if (mirror == null) {
        return false;
    }

    try {
        receiver = mirror.createReceiver(
            Config.channel,
            {
                video: Config.decoder,
            },
            (event) => {
                if (event == Events.Closed) {
                    Log("info", "receiver close callback");
                    Notify("info", "Other devices have turned off screen projection");

                    closeReceiver();
                    baseWindow.hide();
                } else if (event == Events.Initialized) {
                    baseWindow.show();
                }
            }
        );
    } catch (e: any) {
        Log("error", e);
        Notify("error", e.message);

        return false;
    }

    return true;
}

ipcMain.handle(
    "create-sender",
    (_event, sources: { video?: MirrorSourceDescriptor; audio?: MirrorSourceDescriptor }) => {
        Log("info", "ipc create sender event");

        createSender(sources);
    }
);

ipcMain.handle("close-sender", async (_event) => {
    Log("info", "ipc close sender event");

    closeSender();
});

ipcMain.handle("get-settings", () => {
    return { ...Config };
});

ipcMain.handle("set-settings", (_event, settings: typeof Config) => {
    Log("info", "ipc update settings event", settings);

    Config = Object.assign(Config, settings);

    if (createMirror(settings)) {
        createReceiver();
    }
});

ipcMain.handle("get-sources", (_event, kind: MirrorSourceType) => {
    Log("info", "ipc get sources event");

    try {
        return MirrorService.getSources(kind);
    } catch (e: any) {
        Log("error", e);
        Notify("error", e.message);
    }
});

ipcMain.on("close", () => {
    trayWindow.hide();
});

app.on("window-all-closed", () => {
    shutdown();
    app.exit();
});
