import { FontAwesomeIcon } from "@fortawesome/react-fontawesome";
import { faCaretDown, faCaretUp } from "@fortawesome/free-solid-svg-icons";
import styles from "@/styles/settings.module.css";

export const VideoDecoderType: { [key in MirrorVideoDecoderType]: number } = {
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
};

export const VideoEncoderType: { [key in MirrorVideoEncoderType]: number } = {
    /** libx264 (software) */
    X264: 0,
    /** h264_qsv */
    Qsv: 1,
    /** h264_nvenc */
    Cuda: 2,
    /** video tool box */
    VideoToolBox: 3,
};

const Items: {
    [key: string]: {
        key: string;
        type?: "number" | "text";
        element: "input" | "select";
        options?: { [key: string]: number };
    }[];
} = {
    Channel: [
        {
            key: "channel",
            type: "number",
            element: "input",
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
            options: VideoEncoderType,
        },
    ],
    Decoder: [
        {
            key: "decoder",
            element: "select",
            options: VideoDecoderType,
        },
    ],
    Size: [
        {
            key: "width",
            element: "input",
            type: "number",
        },
        {
            key: "height",
            element: "input",
            type: "number",
        },
    ],
    FPS: [
        {
            key: "frameRate",
            element: "input",
            type: "number",
        },
    ],
    BitRate: [
        {
            key: "bitRate",
            element: "input",
            type: "number",
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
        },
    ],
};

export enum SettingsState {
    Hide,
    Min,
    Max,
}

export interface SettingsProps {
    settings?: Settings;
    state: SettingsState;
    onClick?: () => void;
}

export default function Settings({ state, settings, onClick }: SettingsProps) {
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
                    backgroundColor:
                        state == SettingsState.Max ? "rgba(0, 0, 0, 1)" : "rgba(0, 0, 0, 0)",
                }}
            >
                <div id={styles.box}>
                    <div id={styles.switch}>
                        <FontAwesomeIcon
                            icon={faCaretDown}
                            onClick={onClick}
                            style={{
                                display: state == SettingsState.Max ? undefined : "none",
                            }}
                        />
                        <FontAwesomeIcon
                            icon={faCaretUp}
                            onClick={onClick}
                            style={{
                                display: state == SettingsState.Min ? undefined : "none",
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
                                                return <input type={value.type} />;
                                            } else if (value.element == "select") {
                                                return (
                                                    <select>
                                                        {Object.keys(value.options || {}).map(
                                                            (key) => (
                                                                <option
                                                                    value={
                                                                        (value.options || {})[key]
                                                                    }
                                                                >
                                                                    {key}
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
