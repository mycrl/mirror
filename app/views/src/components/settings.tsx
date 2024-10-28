import { FontAwesomeIcon } from "@fortawesome/react-fontawesome";
import { faCaretDown, faGear } from "@fortawesome/free-solid-svg-icons";
import styles from "@/styles/settings.module.css";
import { useEffect, useState } from "react";
import { MirrorVideoDecoderType, MirrorVideoEncoderType } from "mirror-napi";

const Items: {
    [key: string]: {
        key: keyof Settings;
        type?: "number" | "text";
        element: "input" | "select";
        options?: { [key: string]: number };
        into?: (value: any) => any;
    }[];
} = {
    Channel: [
        {
            key: "channel",
            type: "number",
            element: "input",
            into: Number,
        },
    ],
    Server: [
        {
            key: "server",
            type: "text",
            element: "input",
        },
    ],
    Encoder: [
        {
            key: "encoder",
            element: "select",
            options: Object.values(MirrorVideoEncoderType)
                .filter((it) => typeof it == "string")
                .reduce(
                    (value, it) =>
                        Object.assign(value, {
                            [it]: (
                                MirrorVideoEncoderType as unknown as {
                                    [key: string]: MirrorVideoEncoderType;
                                }
                            )[it],
                        }),
                    {}
                ),
            into: Number,
        },
    ],
    Decoder: [
        {
            key: "decoder",
            element: "select",
            options: Object.values(MirrorVideoDecoderType)
                .filter((it) => typeof it == "string")
                .reduce(
                    (value, it) =>
                        Object.assign(value, {
                            [it]: (
                                MirrorVideoEncoderType as unknown as {
                                    [key: string]: MirrorVideoEncoderType;
                                }
                            )[it],
                        }),
                    {}
                ),
            into: Number,
        },
    ],
    Size: [
        {
            key: "width",
            element: "input",
            type: "number",
            into: Number,
        },
        {
            key: "height",
            element: "input",
            type: "number",
            into: Number,
        },
    ],
    FPS: [
        {
            key: "frameRate",
            element: "input",
            type: "number",
            into: Number,
        },
    ],
    BitRate: [
        {
            key: "bitRate",
            element: "input",
            type: "number",
            into: Number,
        },
    ],
    Multicast: [
        {
            key: "multicast",
            element: "input",
            type: "text",
        },
    ],
    MTU: [
        {
            key: "mtu",
            element: "input",
            type: "number",
            into: Number,
        },
    ],
};

export enum SettingsState {
    Hide,
    Min,
    Max,
}

export interface SettingsProps {
    state: SettingsState;
    onClick?: (settings: Settings) => void;
}

export default function Settings({ state, onClick }: SettingsProps) {
    const [settings, setSettings] = useState<Settings>({
        channel: 0,
        server: "127.0.0.1:8080",
        multicast: "239.0.0.1",
        mtu: 1500,
        decoder: MirrorVideoDecoderType.H264,
        encoder: MirrorVideoEncoderType.X264,
        frameRate: 24,
        width: 1280,
        height: 720,
        bitRate: 500 * 1024 * 8,
        keyFrameInterval: 20,
    });

    const getSettings = async () => {
        setSettings(await electronAPI.getSettings());
    };

    const onChanggeSettings = <K extends keyof Settings>(key: K, value: Settings[K]) => {
        setSettings({
            ...settings,
            [key]: value,
        });
    };

    const submitSettings = async () => {
        await electronAPI.setSettings({ ...settings });
        onClick && onClick(settings);
    };

    useEffect(() => {
        return () => {
            getSettings();
        };
    }, []);

    return (
        <>
            <div
                id={styles.settings}
                style={{
                    top:
                        state == SettingsState.Hide
                            ? "400px"
                            : state == SettingsState.Min
                            ? "360px"
                            : 0,
                    backgroundColor: state == SettingsState.Max ? "#fff" : "rgba(0, 0, 0, 0)",
                }}
            >
                <div id={styles.box}>
                    <div id={styles.switch}>
                        <FontAwesomeIcon
                            icon={faCaretDown}
                            onClick={submitSettings}
                            style={{
                                display: state == SettingsState.Max ? undefined : "none",
                                color: "#000",
                            }}
                        />
                        <FontAwesomeIcon
                            icon={faGear}
                            onClick={() => onClick && onClick(settings)}
                            style={{
                                display: state == SettingsState.Min ? undefined : "none",
                                color: "#fff",
                                fontSize: "15px",
                            }}
                        />
                    </div>

                    <div id={styles.items}>
                        {Object.keys(Items).map((key) => {
                            const item = Items[key];

                            return (
                                <div className={styles.item} key={key}>
                                    <div className={styles.key}>
                                        <span>{key}:</span>
                                    </div>
                                    <div className={styles.value}>
                                        {item.map((value) => {
                                            if (value.element == "input") {
                                                return (
                                                    <input
                                                        key={value.key}
                                                        type={value.type}
                                                        value={
                                                            settings[value.key] != null
                                                                ? settings[value.key]
                                                                : ""
                                                        }
                                                        onChange={({ target }) =>
                                                            onChanggeSettings(
                                                                value.key,
                                                                value.into
                                                                    ? value.into(target.value)
                                                                    : target.value
                                                            )
                                                        }
                                                    />
                                                );
                                            } else if (value.element == "select") {
                                                return (
                                                    <select
                                                        key={value.key}
                                                        value={
                                                            settings[value.key] != null
                                                                ? settings[value.key]
                                                                : ""
                                                        }
                                                        onChange={({ target }) =>
                                                            onChanggeSettings(
                                                                value.key,
                                                                value.into
                                                                    ? value.into(target.value)
                                                                    : target.value
                                                            )
                                                        }
                                                    >
                                                        {Object.keys(value.options || {}).map(
                                                            (key) => (
                                                                <option
                                                                    key={key}
                                                                    value={
                                                                        (value.options || {})[key]
                                                                    }
                                                                >
                                                                    {key.toUpperCase()}
                                                                </option>
                                                            )
                                                        )}
                                                    </select>
                                                );
                                            }
                                        })}
                                    </div>
                                </div>
                            );
                        })}
                    </div>
                </div>
            </div>
        </>
    );
}
