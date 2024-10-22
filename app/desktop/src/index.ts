import { app, screen, BrowserWindow, Tray, nativeImage, ipcMain, Menu } from "electron";
import
    {
        Backend,
        MirrorReceiverService,
        MirrorSenderService,
        MirrorService,
        shutdown,
        SourceDescriptor,
        SourceType,
        startup,
        VideoDecoderType,
        VideoEncoderType,
    } from "mirror-napi";
import { join } from "node:path";
import * as fs from "node:fs";

const USER_DATA = app.getPath("userData");
const CONFIG_PATH = join(USER_DATA, "./settings.json");

if (!fs.existsSync(CONFIG_PATH))
{
    fs.writeFileSync(
        CONFIG_PATH,
        JSON.stringify(
            {
                channel: 0,
                server: "127.0.0.1:8080",
                multicast: "239.0.0.1",
                mtu: 1400,
                decoder: VideoDecoderType.D3D11,
                encoder: VideoEncoderType.Qsv,
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

const Config: {
    channel: number;
    server: string;
    multicast: string;
    mtu: number;
    decoder: VideoDecoderType;
    encoder: VideoEncoderType;
    frameRate: number;
    width: number;
    height: number;
    bitRate: number;
    keyFrameInterval: number;
} = new Proxy(JSON.parse(fs.readFileSync(CONFIG_PATH, "utf-8")), {
    get: (target: any, key, _) =>
    {
        return target[key];
    },
    set: (target: any, key, value, _) =>
    {
        target[key] = value;

        fs.writeFileSync(CONFIG_PATH, JSON.stringify(target, null, 4));
        return true;
    },
});

const display = screen.getPrimaryDisplay();
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
        preload: join(__dirname, "../view/preload.js"),
    },
});

window.loadFile(join(__dirname, "../view/index.html"));

const icon = nativeImage.createFromPath(join(__dirname, "../logo.ico"));
const tray = new Tray(icon);

tray.setTitle("mirror");
tray.setToolTip("service is running");
tray.setContextMenu(
    Menu.buildFromTemplate([
        {
            label: "Open DevTools",
            click: () =>
            {
                window.webContents.openDevTools({
                    mode: "detach",
                });
            },
        },
        {
            label: "Exit",
            click: () =>
            {
                app.exit();
            },
        },
    ])
);

const Notify = (level: "info" | "warning" | "error", info: string) =>
{
    tray.displayBalloon({
        title: "mirror",
        content: info,
        iconType: level,
        largeIcon: false,
        icon,
    });

    setTimeout(() =>
    {
        tray.removeBalloon();
    }, 5000);
};

const Log = (level: string, ...args: any[]) =>
{
    console.log(`[${level.toUpperCase()}] - (electron) - `, ...args);
};

tray.on("double-click", (_event, bounds) =>
{
    window.setPosition(bounds.x - 90, display.workAreaSize.height - 420);
    window.show();
});

Notify("info", "The service is running in the background. Double-click the icon to expand it.");

try
{
    Log("info", `startup mirror, user data path=${USER_DATA}`);

    startup(USER_DATA);
} catch (e: any)
{
    Log("error", e);
    Notify("error", e.message);
}

let mirror: MirrorService | null = null;
let sender: MirrorSenderService | null = null;
let receiver: MirrorReceiverService | null = null;

ipcMain.handle(
    "create-sender",
    (_event, sources: { video?: SourceDescriptor; audio?: SourceDescriptor }) =>
    {
        Log("info", "ipc create sender event");

        if (receiver != null)
        {
            Log("info", "receiver is exists, close receiver");

            receiver.destroy();
            receiver = null;
        }

        if (sender == null && mirror != null)
        {
            Log("info", "create sender");

            try
            {
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
                    () =>
                    {
                        Log("info", "sender close callback");

                        if (sender != null)
                        {
                            sender.destroy();
                            sender = null;
                        }

                        Notify("info", "Screen projection has stopped");
                    }
                );
            } catch (e: any)
            {
                Log("error", e);
                Notify("error", e.message);
            }
        } else
        {
            Log("error", "sender is exists");
        }
    }
);

ipcMain.handle("close-sender", async (_event) =>
{
    Log("info", "ipc close sender event");

    if (sender != null)
    {
        Log("info", "close sender");

        sender.destroy();
        sender = null;
    }

    if (receiver == null && mirror != null)
    {
        Log("info", "receiver not exists, create receiver");

        try
        {
            receiver = mirror.createReceiver(
                Config.channel,
                {
                    video: Config.decoder,
                    backend: Backend.Wgpu,
                },
                () =>
                {
                    Log("info", "receiver close callback");

                    if (receiver != null)
                    {
                        receiver.destroy();
                        receiver = null;
                    }

                    Notify("info", "Other devices have turned off screen projection");
                }
            );
        } catch (e: any)
        {
            Log("error", e);
            Notify("error", e.message);
        }
    } else
    {
        Log("warn", "receiver is exists, skip");
    }
});

ipcMain.handle("get-settings", () =>
{
    return { ...Config };
});

ipcMain.handle("set-settings", (_event, settings: typeof Config) =>
{
    Log("info", "ipc update settings event", settings);

    Object.assign(Config, settings);

    if (receiver != null)
    {
        Log("info", "receiver is exists, close receiver");

        receiver.destroy();
        receiver = null;
    }

    Log("info", "rebuild mirror env");

    try
    {
        mirror = new MirrorService(settings);
    } catch (e: any)
    {
        Log("error", e);
        Notify("error", e.message);

        return;
    }

    if (receiver == null)
    {
        Log("info", "receiver not exists, create receiver");

        try
        {
            receiver = mirror.createReceiver(
                Config.channel,
                {
                    video: VideoDecoderType.Qsv,
                    backend: Backend.Wgpu,
                },
                () =>
                {
                    Log("info", "receiver close callback");

                    if (receiver != null)
                    {
                        receiver.destroy();
                        receiver = null;
                    }

                    Notify("info", "Other sources have turned off screen projection");
                }
            );
        } catch (e: any)
        {
            Log("error", e);
            Notify("error", e.message);
        }
    } else
    {
        Log("warn", "receiver is exists, skip");
    }
});

ipcMain.handle("get-sources", (_event, kind: SourceType) =>
{
    Log("info", "ipc get sources event");

    try
    {
        return MirrorService.getSources(kind);
    } catch (e: any)
    {
        Log("error", e);
        Notify("error", e.message);
    }
});

ipcMain.on("close", () =>
{
    window.hide();
});

app.on("window-all-closed", () =>
{
    shutdown();
    app.exit();
});